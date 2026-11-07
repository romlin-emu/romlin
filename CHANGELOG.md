# Changelog

Romlin continues the upstream RomP release history below. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Romlin uses [semantic versioning](https://semver.org/).

## [0.20.0](https://github.com/romlin-emu/romlin/compare/v0.19.1...v0.20.0) (2026-11-07)


### Features

* add textures support for ps2 ([6313775](https://github.com/romlin-emu/romlin/commit/631377505c1b788bec82ada19cf042cc2f8607fb))
* configure PS2 Buzz! and SingStar accessories automatically ([1f41ce3](https://github.com/romlin-emu/romlin/commit/1f41ce3abbfc8e301ecfde5c6e5adac58ea96e19))
* **eden:** configure connected gamepads on launch ([594e0e6](https://github.com/romlin-emu/romlin/commit/594e0e6abee22c3b2742f351dce3b0018d843b3e))
* **steamos:** add one-click RomP installer ([b282332](https://github.com/romlin-emu/romlin/commit/b282332463e3336ef87f6c2b626409400a25574c))


### Bug Fixes

* accept versioned Switch firmware and explain missing downloads ([751393b](https://github.com/romlin-emu/romlin/commit/751393b2a0e862186aec6023789c9cdbaa1f374b))
* allow controller cycling through all six settings tabs ([f9280f8](https://github.com/romlin-emu/romlin/commit/f9280f858e7d9248e47d081921829ea5dc0fa4a0))
* **bios:** continue firmware recovery past stale downloads ([5aac3b1](https://github.com/romlin-emu/romlin/commit/5aac3b1d752ccd79ad7d81fd1114414e0fc4d5d6))
* **eden:** ask to close Eden on Select+Start ([f8aa060](https://github.com/romlin-emu/romlin/commit/f8aa060277f12bac379c13d57587ab4e7e67a186))
* **eden:** ask to close Eden on Select+Start ([1161a71](https://github.com/romlin-emu/romlin/commit/1161a71d685875e3718ec28e096184c9044c31ea))
* **eden:** log controller setup on launch ([54f5e38](https://github.com/romlin-emu/romlin/commit/54f5e38384bea286a032c6789320daedbb7449c0))
* **eden:** make controllers work on SteamOS and keep Eden data across updates ([ee2ee40](https://github.com/romlin-emu/romlin/commit/ee2ee40a2e59e34706b9ff990a15099ecf2e2f5b))
* **gaming-mode:** prepare credentials and normalize UI behavior ([1451821](https://github.com/romlin-emu/romlin/commit/1451821bb24ed5bca62c08c6f01cb02b9735ffb1))
* make controller focus visible and keep settings in view ([6af741a](https://github.com/romlin-emu/romlin/commit/6af741a9ed9c0cecf905f64bea6dd4228f1ebf03))
* **perf:** measure active playback and account for frame loop stalls ([d6b9b56](https://github.com/romlin-emu/romlin/commit/d6b9b560829b723575225a044a2f948c044cf56a))
* **play:** wait for core exit before closing game ([96c50b7](https://github.com/romlin-emu/romlin/commit/96c50b7a5e27aba9f5646a906e599790c01e3053))
* preserve core options and import Eden firmware ([a36352a](https://github.com/romlin-emu/romlin/commit/a36352ae0389fbd44d9385aeaa17256e06c5c1d3))
* ps2 games running on PAL having timing issues ([0cb763c](https://github.com/romlin-emu/romlin/commit/0cb763cf868d5df4bba8f5f7f23eb6c1190e8280))
* **ps2:** start games with the newest retail BIOS, not the first listed ([cfd0a1d](https://github.com/romlin-emu/romlin/commit/cfd0a1d478ec7a46eee7d1ba0e7a3c71521123a9))
* resolve Windows lint and serialize SDL controller tests ([#7](https://github.com/romlin-emu/romlin/issues/7)) ([3e9b9f5](https://github.com/romlin-emu/romlin/commit/3e9b9f5efe050d125b199d23d181350c8dd93b6e))
* **romp-app:** log startup failures ([3077241](https://github.com/romlin-emu/romlin/commit/30772417f63738b98398a543f68ff99c0eb66319))
* **setup:** enable scrolling on short displays ([da91a46](https://github.com/romlin-emu/romlin/commit/da91a46c6d2ccaecca59f39ed32fe46e9fbd86f3))
* **shaders:** compile for advertised GLSL capabilities ([2823234](https://github.com/romlin-emu/romlin/commit/28232348b8e356f807eeffb674269c0b9114a85d))
* **steamos:** detect Gamescope despite overridden desktop labels ([33a8f05](https://github.com/romlin-emu/romlin/commit/33a8f0594043baed5e4d4beb05ce8ebe2f3af2dc))
* **steamos:** install the nightly AppImage ([426a6c7](https://github.com/romlin-emu/romlin/commit/426a6c7e8894d0d5cb7689884d3c453c289eef0a))
* **ui:** navigate and reveal game page controls with a controller ([0439532](https://github.com/romlin-emu/romlin/commit/0439532e737f4fddac3de2456ba13a39a067b2b6))
* **vulkan:** prefer cached readback memory on integrated GPUs ([4fdf052](https://github.com/romlin-emu/romlin/commit/4fdf05262f977e87b3b1a791f0fc0a2607e1b45e))
* **vulkan:** prefer capable hardware GPUs with core fallback ([2eff14a](https://github.com/romlin-emu/romlin/commit/2eff14a1b3933e067b150dad387bcb724bd856d4))
* **vulkan:** retain system memory preference within readback tiers ([52f268f](https://github.com/romlin-emu/romlin/commit/52f268f144ffbfd05f9094bea6e9bd6c9de0d514))

## [0.19.1](https://github.com/romlin-emu/romlin/compare/v0.19.0...v0.19.1) (2026-11-04)


### Bug Fixes

* **eden:** make controllers work on SteamOS and keep Eden data across updates ([164efd5](https://github.com/romlin-emu/romlin/commit/164efd53d67b542f4249d759be03c15495511dea))
* resolve Windows lint and serialize SDL controller tests ([#7](https://github.com/romlin-emu/romlin/issues/7)) ([bb2e343](https://github.com/romlin-emu/romlin/commit/bb2e3438f1889b700a8da2276221273dc120a13f))

## [Unreleased]

### Added

- Nintendo Switch games can launch in Eden on Linux and Windows (x86-64 and ARM64). RomP installs a checksum-verified Eden build and imports your `prod.keys`, optional `title.keys`, and `firmware.zip` from the Switch firmware in RomM. Eden saves and user profiles sync to RomM per game, including offline retries and conflict handling; firmware and keys stay local.

- Games on more than one disc can change disc from the game menu, when the game asks for the next one. RomP opens the tray, swaps the disc and closes it, as you would on the console. Discs are ordered by the number in their names, as RomM orders them, so Disc 10 comes after Disc 2. Amiga games whose disks are each zipped get the same disk list, starting from the first disk, instead of starting from the largest zip.
- Save states can be deleted from the game's page, including where you left off. Each asks first, keeps a backup in the game's save folder, and is deleted on RomM too, unless another device has saved a newer one there since.

### Fixed

- PS2 in-game saves persist after closing and reopening the game. RomP now reports successful file seeks correctly, so the emulator writes memory-card data and its checksum to disk.
- PS2 games on Windows and Linux no longer crash on start when the BIOS folder holds Japan v1.80 or v2.10. RomP now starts them with the newest retail BIOS it finds, and skips those two, instead of whichever file the emulator listed first.
- A mouse button let go over the game menu no longer stays held in the game. Held through Restart, it stopped Amiga games booting from their disk.
- Amiga games made for the Amiga 1200, with "AGA" in their name, start on an Amiga 1200 instead of an Amiga 600, which can't run them. Others with the model in their name, such as "(A500)", get that model too, and the rest still start on an Amiga 600.
- After the computer's clock was set back, the backup RomP makes of a save before replacing it could be removed straight away instead of kept with the newest ten.

## [0.19.0] - 2026-10-24

### Added

- RomP keeps a log of each of its last five launches, including what each game's emulator reported, in a `logs` folder beside your saves. Each game's part starts at a "Playing" line and ends at a "Stopped playing" line saying how it ended. About RomP → Show log opens the folder, and Copy the last game's log copies just that game's part, to paste into a bug report. The log stays on your computer. Emulators' debug messages are left out unless RomP is started with `RUST_LOG=debug`.

### Fixed

- DOS games' in-game saves sync with RomM, so they carry over to and from Argosy. DOSBox Pure keeps them in a `.pure.zip` of its own rather than in save memory, which RomP didn't pick up.

## [0.18.1] - 2026-10-20

### Fixed

- The sound no longer speeds up and slows down for the first minute of a game, or after pausing it or switching away from its window. RomP now starts the sound buffer full enough and tops it up again on resume, and keeps its corrections too small to hear.
- Games no longer sound warbly on Windows when the emulator's sound rate isn't one the sound card uses, such as mGBA's 131072 Hz: RomP converts it to the card's own rate itself.
- PlayStation and Nintendo 64 games run at full speed on graphics cards that let the processor see their memory, such as AMD cards with Smart Access Memory, instead of a couple of frames a second.

## [0.18.0] - 2026-10-18

### Added

- PICO-8 carts play, `.p8` and `.p8.png`, on the FAKE-08 core. RomP downloads it from RompEmu's own build of FAKE-08 for macOS, Windows and Linux, since libretro doesn't build it. Carts keep their own saved progress, but there are no save states yet: FAKE-08's would bring back the screen without the game.

## [0.17.0] - 2026-10-17

### Added

- RomP can be chosen under Install on device in RomM 5.4: a game sent to it starts downloading straight away, or the next time RomP is open, with a card in the library showing its progress and a Play button when it's ready, and RomM hears whether it worked. Turn it off under Settings › RomM › Accept games sent from RomM.
- RomM 5.4's notifications, such as a finished library scan, show behind a bell in the sidebar, checked every minute. Opening them marks them read, and each can be dismissed.
- The console filter also finds consoles by the short and other names RomM 5.4 gives them, such as MD or Mega Drive for the Genesis.
- Getting back to the whole library is one step: "Clear filters" beside the heading, or in an empty view, clears the search and console filter and returns to All games, as do the RomP title in the sidebar and a second Esc in either field. Both fields also have a × to clear them.
- RomM 5.4 learns which version of each in-game save RomP holds, so its web player and other devices can tell a save unchanged here from one changed in two places.

### Fixed

- Creating or renaming a collection to a name already taken, or one that's too long, says so, instead of "the server has a newer version".

### Security

- Archives RomP unpacks can no longer place links or privileged files outside their folder (zip 9: RUSTSEC-2026-0367, RUSTSEC-2026-0368, RUSTSEC-2026-0369).

## [0.16.0] - 2026-10-16

### Added

- Games for 20 more systems play: Atari 5200, 7800, 8-bit computers and ST, Amstrad CPC, Commodore VIC-20, Plus/4, 16 and 128, Amiga CD32 and CDTV, Neo Geo CD, PC-FX, Pokémon Mini, Game & Watch, Watara Supervision, Mega Duck, Fairchild Channel F and TIC-80, plus Atari Jaguar CD on the Virtual Jaguar core that already plays Jaguar cartridges. RomP downloads each one's emulator on first play, and the BIOS files the Atari ST, Neo Geo CD, PC-FX, CD32, CDTV and Channel F need from RomM's firmware.
- A filter at the top of the sidebar finds consoles by name or RomM's short name, ignoring case and accents. It works alongside Downloaded only, and the console you picked stays selected and named while the filter hides it.
- A game RomM keeps in another emulator's own format, such as BigPEmu's .bigpimg, says so before downloading, along with the formats that would play. When an emulator can't play a game's file type, RomP now says which types it plays instead of passing the emulator a file it can't read.

### Fixed

- Zipped ZX Spectrum games start instead of stopping at the Sinclair screen. Emulators that open zips but don't start what's in them now get the game unpacked, which also lets zipped Atari 8-bit and Amstrad CPC games boot.
- Games that are archives under another name, such as Game & Watch games, download instead of failing as corrupted.
- With the Platforms section collapsed, the library's heading names the selected console instead of saying "Games".
- Amiga games run with the immediate blitter RomP meant to turn on: the setting used a value PUAE doesn't accept, so it never applied.

## [0.15.0] - 2026-10-12

### Added

- RomP comes as a .deb for Debian and Ubuntu and an .rpm for Fedora and openSUSE, beside the AppImage. They install the libraries RomP needs, and when a newer RomP is out, it offers the same kind of package.
- The AppImage describes RomP to Linux software centers and AppImage catalogs, with a summary, screenshots and links.

### Fixed

- The Linux AppImage starts on systems without libxkbcommon-x11, such as minimal installs, instead of closing straight away. It now brings its own copy.

## [0.14.0] - 2026-10-12

### Added

- RomP tells you when a newer version is out, with a link to the download for your computer and to what's new. It checks GitHub at most once a day, and you can skip a version or turn the check off in Settings. When Homebrew installed RomP, it says to run `brew upgrade romp` instead.

## [0.13.1] - 2026-10-11

### Fixed

- A game whose save didn't come down from RomM before 0.13.0, and that has since started a fresh save, is no longer stuck with it: when the save here and RomM's differ and RomP can't tell which is newer, it asks which to keep, as it does for any conflict. RomP now decides each sync from the save here, its newest on RomM and the one it last synced, instead of relying on RomM's sync answer, which can miss a save or offer another emulator's.

## [0.13.0] - 2026-10-08

### Added

- A game's page shows its in-game save beside its save states, and whether it's synced with RomM, waiting to sync, or on RomM and coming down when you play.
- When RomM only has a game's in-game save from another emulator, such as another app's GBA save, the game's page offers to use it. RomP uses another emulator's save only when you choose it there. This is offered for NES, SNES, Game Boy, Game Boy Advance, Genesis, Master System, Game Gear, DS, PlayStation and PS2 games, whose saves carry over between emulators.

### Fixed

- GameCube and Wii games show their picture, instead of a black screen with sound.
- A game whose in-game save is on RomM but not on this computer gets it when it starts, including a save this computer uploaded before, instead of starting with no save or with an older one from another app.

## [0.12.0] - 2026-10-07

### Added

- RomP speaks French, Spanish, German and Arabic, translated on [Crowdin](https://crowdin.com/project/romp). It follows the system's language, or the one picked in Settings → General → Language. In Arabic the interface reads right to left.

### Changed

- Text uses the Inter font, the same on every system, and IBM Plex Sans Arabic in Arabic.

### Fixed

- Saturn games downloaded as a zip of a CD image load their disc, instead of the Saturn starting with an empty drive.

### Security

- xemu and RPCS3 downloads are checked against the checksum GitHub publishes for them, and the Xbox hard disk image against a known one; a download that doesn't match isn't installed. By @gantoine in [#12](https://github.com/RompEmu/RomP/pull/12).
- The Linux AppImage is built with fixed, checksum-checked versions of appimagetool and its runtime, instead of their latest builds. By @gantoine in [#11](https://github.com/RompEmu/RomP/pull/11).

## [0.11.1] - 2026-10-04

### Fixed

- The Linux AppImage and tarball start on Linux systems from Ubuntu 22.04 and Debian 12 on, instead of needing the newest ones. The AppImage is now named without "linux", as AppImages are.

## [0.11.0] - 2026-10-04

### Added

- Controller hints, the button list in Settings and the game menu use the controller's own names: A and B on Xbox controllers, the Cross and Circle symbols on PlayStation ones, A and B in their places on Nintendo ones.
- Each controller can be given another kind's button labels, in its button list in Settings, for controllers that say they're another kind, such as 8BitDo's in their macOS mode.

### Changed

- Menus confirm with the bottom button and go back with the right one, as Steam does, except on Nintendo controllers, which keep A on the right. Menus also stay put when a game's buttons are remapped.

### Fixed

- Save states can be saved and loaded with a controller in the game menu: the controller moves between each slot's Save and Load buttons and presses the chosen one.
- Matching button labels on Nintendo systems no longer swaps A and B on Nintendo controllers, which already have them in place.

- Controllers that macOS looks after itself, such as the Switch Pro Controller over Bluetooth, now work in games and menus instead of showing as connected but doing nothing. RomP reads controllers through SDL now, which also knows more controllers on every system. Button layouts and player choices carry over, except for Xbox controllers on Windows, which need their player chosen again once.

## [0.10.0] - 2026-10-02

### Added

- Command-W on a Mac, or Control-W on Windows and Linux, closes the game, the same as its window's close button.

### Changed

- Hardcore mode is off, and its setting hidden, until RetroAchievements adds RomP to its approved emulators. RetroAchievements doesn't accept hardcore unlocks from emulators it hasn't approved, and only considers ones that have been public for six months.
- Nintendo DS games play in melonDS DS instead of DeSmuME. Their saves now sync with RomM as .sav files, the same as standalone melonDS. Existing DeSmuME saves become melonDS saves the first time RomP looks at them; DeSmuME's save and save states, which melonDS can't load, are kept in the game's backup folder.
- PSP games no longer offer the handheld screen look or its colours. The PSP draws at several times its screen's resolution, where the screen's pixel grid turned into a pattern.
- Consoles without a chosen look start with sharp pixels, even if the Sharp pixels switch removed in 0.9.0 was off.

### Fixed

- Play from here starts from the chosen save state even if Play is pressed while the game is getting ready.
- Play from here is off in hardcore mode, with a note saying so, instead of quietly starting from the beginning.
- The CRT's TV screen looks the same after switching to Monitor and back during a game.
- Games show their picture plainly instead of a black window on computers where shaders can't run.
- Games play without sound instead of failing to start on computers with no sound output, such as over Remote Desktop.
- On Windows, games in a zip or 7z now load on cores that need the file itself, such as VICE and DOSBox Pure.
- Game Boy Color games show their colours on the handheld screen instead of a Game Boy Pocket's olive tint.
- Atari 2600 games load again with the current Stella, which only finds games through the file access RetroArch gives cores. RomP now gives cores that too.
- A Mac no longer dozes off or slows the game window down while you play with a controller, which macOS doesn't count as using the computer.

## [0.9.0] - 2026-10-02

### Added

- Save states with pictures. Save states in the game menu shows your four slots with a picture of the moment you saved and when that was, and the game's page lists your save states with a Play from here button. Pictures sync with RomM along with the states.
- Play time on each game's page: how long you've played across every device, how many sessions, and when you last played. Romp asks you once to pair again with RomM for the permission this needs.
- Looks for every console: a CRT, the screen of each handheld, sharp pixels that stay evenly sized at any window size, or smooth pixels, drawn with the best shaders from RetroArch's collection. Each console starts with the screen it was played on, and Look in the game menu changes it live. The CRT can be a TV or a monitor, with settings for curvature, scanlines and mask.
- Genesis, Sega CD, 32X and Saturn games blend their checkerboard dithering into the transparency it stood for on a TV.
- Game Boy Color, Game Boy Advance, DS and PSP games show the colours their screens did, and Game Boy games can show a Game Boy Pocket screen, the original green one, or a modern backlit one.
- A Looks tab in Settings to choose each console's look outside a game.
- Achievement popups can be quiet or off, in Settings or from the Achievements panel in the game menu. Quiet keeps unlock popups and shows progress only at 25%, 50% and 75%, at most once a minute; everything else, or everything when off, is a brief note in the corner. Popups and notes can sit in any corner of the screen.
- The Achievements panel in the game menu lists what you unlocked and how far along you got while playing first.

### Changed

- Achievement popups are quiet by default and appear in the top-right corner instead of the top centre.
- The Sharp pixels switch in Settings is replaced by each console's look in the Looks tab, where the Xbox's sharp or smooth scaling is set too.

## [0.8.0] - 2026-10-01

### Added

- RetroAchievements. Sign in under Settings → Account to earn achievements as you play, with a popup and badge for each unlock. Romp keeps RetroAchievements' sign-in token in the keychain, never your password, and only the app talks to RetroAchievements, never the sandboxed emulator.
- An Achievements list in the game menu, and an Achievements section on each game's page with every badge and what you've earned.
- Leaderboards, with a counter in the corner of the game window during an attempt, along with challenge icons and progress toward an achievement.
- Hardcore mode, under Settings → Account. While it's on, games start fresh and save states can't be loaded, and an emulator setting RetroAchievements doesn't allow turns it off for that game with a note saying why. RetroAchievements counts hardcore unlocks from Romp once it has reviewed Romp; until then they count as casual.
- Save states remember achievement progress, and achievements earned while RetroAchievements can't be reached are sent later, even after Romp restarts.

### Changed

- After you play, RomM refreshes your RetroAchievements progress. If you're signed in to RetroAchievements, Romp asks you once to pair again with RomM for the permission this needs.

## [0.7.0] - 2026-09-30

### Added

- PS3 games play through RPCS3, which Romp downloads the first time along with the PS3 system software from RomM's firmware. Games can be folders or ISOs, or a zip or 7z of one, which Romp unpacks after downloading. RPCS3 follows Romp's fullscreen, volume and controller settings, and keeps its own settings and saves apart from any RPCS3 you installed yourself.
- RomM learns how long you play each game, and shows the game you're playing right now. Play time recorded offline uploads the next time Romp reaches the server.
- A "For you" list in the sidebar with games RomM recommends from what you've played. A recommended game's page says why it was picked.
- Screenshots. Press F12 or choose Take screenshot in the game menu. Romp keeps them with the game's saves, uploads them to your RomM gallery, and shows them first on the game's page.
- The keyboard shortcuts for pause, save and load state, save slots, full screen and screenshots can be changed under Settings → Players → Shortcuts, including to combinations such as Ctrl+S. Esc always opens the game menu.

### Changed

- On Windows and Linux, GameCube, Wii, PSP, Dreamcast, PlayStation, N64 and PS2 games draw through Vulkan when the computer has a working Vulkan driver, and through OpenGL otherwise. N64 gets the accurate paraLLEl-RDP renderer, and the resolution settings in the Consoles tab.
- Dependencies are up to date.

### Fixed

- PlayStation games drawn through Vulkan show the right colors again instead of a doubled, green picture. Newer builds of the PlayStation emulator hand over 16-bit and other image formats, which Romp now converts.
- Games RomM hasn't hashed, such as large PS3 ISOs, no longer fail to download with "the downloaded file is corrupted". Romp checks their size instead.

## [0.6.0] - 2026-09-29

### Added

- A Licenses tab in the About window, with the license of Romp and of every emulator it downloads.
- Resolution settings on Macs for N64, GameCube and Wii, PlayStation, PSP and Dreamcast, and a 4x choice for PS2. They default to 2x, or 3x for PSP.
- PS2 memory cards sync with RomM like other in-game saves, and follow a game between ARMSX2 on a Mac and PCSX2 on Windows or Linux.

### Changed

- The About window is roomier and easier to read, and says who makes Romp.
- The audio library and other dependencies are up to date.

### Fixed

- PS2 games on Windows and Linux can save to their memory card. Each game now gets its own card in its save folder.
- The last options on long Settings tabs are no longer cut off.

## [0.5.0] - 2026-09-29

### Changed

- PS2 games play with ARMSX2 on Macs, in place of Play!. It needs a PS2 BIOS in the platform's firmware on your RomM server.
- Romp updates ARMSX2 when a newer build is published.
- PS2 games on Macs render at three times their native resolution, with 16x anisotropic filtering.
- Save states are stored at their real size. ARMSX2's shrink from 68 MB to a few MB.
- On Macs, GameCube, Wii, PSP, Dreamcast and PlayStation games draw through Vulkan instead of Apple's outdated OpenGL.
- N64 games on Macs use the accurate paraLLEl-RDP renderer through Vulkan, in place of the slow software renderer.

### Added

- A Consoles tab in Settings, starting with PS2 resolution, anisotropic filtering and widescreen patches.

### Fixed

- PS2 games on Macs can save to their memory card, and pick up where you left off.

## [0.4.0] - 2026-09-28

### Added

- Linux releases include an AppImage alongside the tarball.
- Games are sandboxed on Windows: the emulator can't start other programs or use the clipboard, and can only write to the game's save folder.
- Game windows open where you last left them, with their size, for each console. The Nintendo DS touch screen window is remembered separately.

### Changed

- Esc closes the About and remap windows. While a key is being remapped, Esc cancels just that key.

## [0.3.0] - 2026-09-28

Tagged but not published as a download. Its changes ship in 0.4.0.

### Added

- Windows builds, published with every release and nightly.
- PS2 games play with PCSX2 on Windows and Linux. It needs a PS2 BIOS in the platform's firmware on your RomM server. Macs keep using Play!.

### Changed

- On Windows, games and saves are stored in `%LOCALAPPDATA%\Romp`.

### Fixed

- Games no longer close as soon as they start on Windows.

## [0.2.0] - 2026-09-28

### Added

- A short welcome guide on first launch that helps you connect to your RomM server.

## [0.1.0] - 2026-09-28

First release, for macOS and Linux.

### Added

- Connect to a RomM 5.0 or newer server and approve Romp by scanning a code or opening a link.
- Browse your library by platform, favorites, collections and recently played, with search, sorting and covers in each console's box shape.
- Game pages with details, screenshots and similar games, and editing of favorites and collections.
- Download games to play them, including offline, with resumable, verified downloads and multi-disc sets.
- Emulator cores and BIOS files installed automatically, with each game running in its own sandboxed process.
- Original Xbox games through xemu.
- In-game saves and save states synced with RomM before and after play, with a choice when both sides changed.
- An in-game menu with pause, restart, save slots, volume and full screen.
- Multiple players with gamepads, remappable keys and buttons, and controller navigation of the whole app.
- Keyboard and mouse for computer systems, and light guns and other accessories per port.
- Two windows for Nintendo DS screens, and rotated screens for vertical arcade games.
- Interface sizes of 1x, 1.5x and 2x, and the window, last view and scroll position remembered between runs.

[Unreleased]: https://github.com/RompEmu/RomP/compare/v0.19.0...HEAD
[0.19.0]: https://github.com/RompEmu/RomP/compare/v0.18.1...v0.19.0
[0.18.1]: https://github.com/RompEmu/RomP/compare/v0.18.0...v0.18.1
[0.18.0]: https://github.com/RompEmu/RomP/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/RompEmu/RomP/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/RompEmu/RomP/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/RompEmu/RomP/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/RompEmu/RomP/compare/v0.13.1...v0.14.0
[0.13.1]: https://github.com/RompEmu/RomP/compare/v0.13.0...v0.13.1
[0.13.0]: https://github.com/RompEmu/RomP/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/RompEmu/RomP/compare/v0.11.1...v0.12.0
[0.11.1]: https://github.com/RompEmu/RomP/compare/v0.11.0...v0.11.1
[0.11.0]: https://github.com/RompEmu/RomP/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/RompEmu/RomP/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/RompEmu/RomP/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/RompEmu/RomP/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/RompEmu/RomP/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/RompEmu/RomP/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/RompEmu/RomP/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/RompEmu/RomP/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/RompEmu/RomP/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/RompEmu/RomP/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/RompEmu/RomP/releases/tag/v0.1.0
