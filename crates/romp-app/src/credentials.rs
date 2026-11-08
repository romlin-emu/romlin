#[cfg(target_os = "linux")]
mod secret_service;

#[cfg(target_os = "linux")]
pub fn prepare(game_mode: bool) {
    if game_mode {
        if let Err(error) = secret_service::prepare() {
            tracing::warn!(error = %format_args!("{error:#}"), "could not prepare the system credential store");
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn prepare(_game_mode: bool) {}

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub trait TokenStore: Send + Sync {
    fn load(&self, account: &str) -> Option<String>;
    fn save(&self, account: &str, token: &str) -> Result<(), String>;
    fn delete(&self, account: &str);
}

pub trait Secrets: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String>;
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String>;
    fn delete(&self, service: &str, account: &str);
}

pub struct Keychain;

impl Secrets for Keychain {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(service, account).and_then(|e| e.get_password()) {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => {
                let error = error_cause(&error);
                tracing::warn!(service, %error, "could not read saved sign-in");
                Err(error.to_string())
            }
        }
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(service, account)
            .and_then(|e| e.set_password(value))
            .map_err(|error| {
                let error = error_cause(&error);
                tracing::warn!(service, %error, "could not store saved sign-in");
                error.to_string()
            })
    }

    fn delete(&self, service: &str, account: &str) {
        if let Ok(entry) = keyring::Entry::new(service, account) {
            let _ = entry.delete_credential();
        }
    }
}

fn error_cause(error: &keyring::Error) -> &keyring::Error {
    // Entry::new replaces initialization failures with NoDefaultStore.
    keyring::Entry::store_status()
        .as_ref()
        .err()
        .unwrap_or(error)
}

const SERVICE: &str = "Romp";
const ACCOUNT: &str = "sign-ins";

type Tokens = BTreeMap<String, String>;

#[derive(Clone, Copy)]
enum Kind {
    RomM,
    RetroAchievements,
}

impl Kind {
    fn key(self, account: &str) -> String {
        match self {
            Kind::RomM => format!("romm:{account}"),
            Kind::RetroAchievements => format!("ra:{account}"),
        }
    }

    /// The items sign-ins were kept in before they shared one, newest first. The service names
    /// stay as RomP chose them so existing sign-ins remain available.
    fn legacy_services(self) -> &'static [&'static str] {
        match self {
            Kind::RomM => &["Romp", "Cartridge"],
            Kind::RetroAchievements => &["Romp RetroAchievements"],
        }
    }
}

/// Every sign-in lives in one keychain item, read once, so a rebuilt app asks for the keychain password once.
pub struct SignIns {
    secrets: Box<dyn Secrets>,
    cache: Mutex<Option<Tokens>>,
}

impl SignIns {
    pub fn new(secrets: impl Secrets + 'static) -> Arc<SignIns> {
        Arc::new(SignIns {
            secrets: Box::new(secrets),
            cache: Mutex::new(None),
        })
    }

    pub fn romm(self: &Arc<Self>) -> Arc<dyn TokenStore> {
        Arc::new(SignInsOf {
            sign_ins: self.clone(),
            kind: Kind::RomM,
        })
    }

    pub fn retro_achievements(self: &Arc<Self>) -> Arc<dyn TokenStore> {
        Arc::new(SignInsOf {
            sign_ins: self.clone(),
            kind: Kind::RetroAchievements,
        })
    }

    /// A keychain that can't be read is an error, never an empty list, so a save can't overwrite the other sign-ins.
    fn stored(&self, cache: &mut Option<Tokens>) -> Result<Tokens, String> {
        if let Some(tokens) = cache {
            return Ok(tokens.clone());
        }
        let tokens: Tokens = self
            .secrets
            .get(SERVICE, ACCOUNT)?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        *cache = Some(tokens.clone());
        Ok(tokens)
    }

    fn store(&self, cache: &mut Option<Tokens>, tokens: Tokens) -> Result<(), String> {
        if tokens.is_empty() {
            self.secrets.delete(SERVICE, ACCOUNT);
        } else {
            let json = serde_json::to_string(&tokens).map_err(|e| e.to_string())?;
            self.secrets.set(SERVICE, ACCOUNT, &json)?;
        }
        *cache = Some(tokens);
        Ok(())
    }

    fn load(&self, kind: Kind, account: &str) -> Option<String> {
        let mut cache = self.cache.lock().unwrap();
        let mut tokens = self.stored(&mut cache).ok()?;
        let key = kind.key(account);
        if let Some(token) = tokens.get(&key) {
            return Some(token.clone());
        }
        let (service, token) = kind.legacy_services().iter().find_map(|service| {
            let token = self.secrets.get(service, account).ok()??;
            Some((*service, token))
        })?;
        tokens.insert(key, token.clone());
        if self.store(&mut cache, tokens).is_ok() {
            self.secrets.delete(service, account);
        }
        Some(token)
    }

    fn save(&self, kind: Kind, account: &str, token: &str) -> Result<(), String> {
        let mut cache = self.cache.lock().unwrap();
        self.stored(&mut cache)
            .and_then(|mut tokens| {
                tokens.insert(kind.key(account), token.into());
                self.store(&mut cache, tokens)
            })
            .map_err(|e| tr::tr!("Could not store the sign-in in the keychain: {e}", e))
    }

    fn delete(&self, kind: Kind, account: &str) {
        let mut cache = self.cache.lock().unwrap();
        if let Ok(mut tokens) = self.stored(&mut cache) {
            if tokens.remove(&kind.key(account)).is_some() {
                let _ = self.store(&mut cache, tokens);
            }
            for service in kind.legacy_services() {
                self.secrets.delete(service, account);
            }
        }
    }
}

struct SignInsOf {
    sign_ins: Arc<SignIns>,
    kind: Kind,
}

impl TokenStore for SignInsOf {
    fn load(&self, account: &str) -> Option<String> {
        self.sign_ins.load(self.kind, account)
    }

    fn save(&self, account: &str, token: &str) -> Result<(), String> {
        self.sign_ins.save(self.kind, account, token)
    }

    fn delete(&self, account: &str) {
        self.sign_ins.delete(self.kind, account);
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct MemorySecrets {
    items: Arc<Mutex<std::collections::HashMap<(String, String), String>>>,
    reads: Arc<std::sync::atomic::AtomicUsize>,
    locked: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl MemorySecrets {
    fn item(&self, service: &str, account: &str) -> Option<String> {
        let key = (service.to_string(), account.to_string());
        self.items.lock().unwrap().get(&key).cloned()
    }

    fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn lock(&self, locked: bool) {
        self.locked
            .store(locked, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
impl Secrets for MemorySecrets {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("locked".into());
        }
        Ok(self.item(service, account))
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        let key = (service.to_string(), account.to_string());
        self.items.lock().unwrap().insert(key, value.into());
        Ok(())
    }
    fn delete(&self, service: &str, account: &str) {
        let key = (service.to_string(), account.to_string());
        self.items.lock().unwrap().remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn unavailable_store_reports_its_initialization_error() {
        const CHILD: &str = "ROMLIN_TEST_KEYRING_FAILURE";
        if std::env::var_os(CHILD).is_some() {
            tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(std::io::stderr)
                .init();
            let tokens = SignIns::new(Keychain).romm();
            assert_eq!(tokens.load("https://debug.invalid/"), None);
            let error = tokens
                .save("https://debug.invalid/", "ROMLIN_TEST_SECRET")
                .unwrap_err();
            let cause = keyring::Entry::store_status().as_ref().unwrap_err();
            assert!(error.contains(&cause.to_string()), "{error}");
            return;
        }

        // A fresh process isolates keyring's cached initialization from other tests.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "credentials::tests::unavailable_store_reports_its_initialization_error",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/")
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{log}");
        assert!(log.contains("could not read saved sign-in"), "{log}");
        assert!(log.contains("Platform failure:"), "{log}");
        assert!(!log.contains("No default store has been set"), "{log}");
        assert!(!log.contains("ROMLIN_TEST_SECRET"), "{log}");
    }

    #[test]
    fn a_sign_in_saved_under_the_oldest_name_moves_into_the_one_item() {
        let secrets = MemorySecrets::default();
        secrets.set("Cartridge", "http://a/", "old").unwrap();
        let tokens = SignIns::new(secrets.clone()).romm();
        assert_eq!(tokens.load("http://a/").as_deref(), Some("old"));
        assert_eq!(secrets.item("Cartridge", "http://a/"), None);
        tokens.delete("http://a/");
        secrets.set("Cartridge", "http://a/", "stale").unwrap();
        tokens.delete("http://a/");
        assert_eq!(tokens.load("http://a/"), None);
    }

    #[test]
    fn sign_ins_in_the_old_items_move_into_one() {
        let secrets = MemorySecrets::default();
        secrets.set("Romp", "http://a/", "romm").unwrap();
        secrets
            .set("Romp RetroAchievements", "beshr", "ra")
            .unwrap();
        let sign_ins = SignIns::new(secrets.clone());
        assert_eq!(sign_ins.romm().load("http://a/").as_deref(), Some("romm"));
        assert_eq!(
            sign_ins.retro_achievements().load("beshr").as_deref(),
            Some("ra")
        );
        assert_eq!(secrets.item("Romp", "http://a/"), None);
        assert_eq!(secrets.item("Romp RetroAchievements", "beshr"), None);

        let reopened = SignIns::new(secrets.clone());
        assert_eq!(reopened.romm().load("http://a/").as_deref(), Some("romm"));
        assert_eq!(
            reopened.retro_achievements().load("beshr").as_deref(),
            Some("ra")
        );
    }

    #[test]
    fn the_keychain_is_read_once() {
        let secrets = MemorySecrets::default();
        let sign_ins = SignIns::new(secrets.clone());
        sign_ins.romm().save("http://a/", "romm").unwrap();
        sign_ins.retro_achievements().save("beshr", "ra").unwrap();

        let reopened = SignIns::new(secrets.clone());
        let before = secrets.reads();
        for _ in 0..3 {
            assert_eq!(reopened.romm().load("http://a/").as_deref(), Some("romm"));
            assert_eq!(
                reopened.retro_achievements().load("beshr").as_deref(),
                Some("ra")
            );
        }
        assert_eq!(secrets.reads() - before, 1);
    }

    #[test]
    fn a_keychain_that_cant_be_read_is_left_alone() {
        let secrets = MemorySecrets::default();
        SignIns::new(secrets.clone())
            .romm()
            .save("http://a/", "romm")
            .unwrap();
        let stored = secrets.item(SERVICE, ACCOUNT);

        secrets.lock(true);
        let sign_ins = SignIns::new(secrets.clone());
        assert_eq!(sign_ins.romm().load("http://a/"), None);
        assert!(sign_ins.retro_achievements().save("beshr", "ra").is_err());
        sign_ins.romm().delete("http://a/");
        assert_eq!(secrets.item(SERVICE, ACCOUNT), stored);

        secrets.lock(false);
        assert_eq!(sign_ins.romm().load("http://a/").as_deref(), Some("romm"));
    }

    #[test]
    fn sign_ins_are_per_kind_and_account() {
        let secrets = MemorySecrets::default();
        let sign_ins = SignIns::new(secrets.clone());
        sign_ins.romm().save("same", "one").unwrap();
        sign_ins.romm().save("other", "two").unwrap();
        sign_ins.retro_achievements().save("same", "three").unwrap();
        sign_ins.romm().delete("same");
        assert_eq!(sign_ins.romm().load("same"), None);
        assert_eq!(sign_ins.romm().load("other").as_deref(), Some("two"));
        assert_eq!(
            sign_ins.retro_achievements().load("same").as_deref(),
            Some("three")
        );
    }

    #[test]
    fn signing_out_of_everything_removes_the_item() {
        let secrets = MemorySecrets::default();
        let sign_ins = SignIns::new(secrets.clone());
        sign_ins.romm().save("http://a/", "romm").unwrap();
        sign_ins.retro_achievements().save("beshr", "ra").unwrap();
        sign_ins.romm().delete("http://a/");
        assert!(secrets.item(SERVICE, ACCOUNT).is_some());
        sign_ins.retro_achievements().delete("beshr");
        assert_eq!(secrets.item(SERVICE, ACCOUNT), None);
    }
}
