# Steam artwork

Generated with the built-in imagegen tool. The existing
`crates/romp-app/assets/icon.png` was the reference for the mascot artwork;
`icon.png` is a copy of that existing icon. ImageMagick resized the generated
images for Steam; the logo retains its generated alpha channel.

Files: capsule.png (600×900), header.png (920×430), hero.png (1920×620),
logo.png (1280×427, transparent), icon.png (1024×1024).

Final prompts, in capsule, hero, header, logo order:

> Use case: ads-marketing. Asset type: Steam portrait library capsule, 600x900 pixels. Create polished artwork for RomP, a retro game library player. Reference image: existing RomP app icon, preserve its smiling orange pixel coin mascot and cyan glass orb identity. Dark charcoal background with subtle cyan glow and restrained pixel sparks. Large crisp white text exactly "RomP" at top, central orange pixel mascot floating inside cyan glass orb, generous margins. Premium playful retro aesthetic. No other text, no third-party characters, no watermark. Portrait 2:3 aspect ratio.

> Use case: ads-marketing. Asset type: Steam library hero wide banner 1920x620. Reference image: existing RomP app icon for mascot identity. Dark charcoal cinematic background with subtle cyan pixel spark field. On right half, the smiling orange pixel coin mascot within a luminous cyan glass orb. Left half quiet dark negative space for Steam interface logo overlay. Polished playful retro gaming artwork. No text, no logos, no third-party characters or watermark. Very wide 3.1:1 composition; keep subject fully inside middle vertical safe area.

> Use case: ads-marketing. Asset type: Steam landscape library capsule 920x430. Reference image existing RomP icon, preserve smiling orange pixel coin inside cyan glass orb. Dark charcoal background subtle cyan pixel sparks. Large white exact text "RomP" on left, full mascot orb on right. Clean readable at thumbnail sizes. Wide 92:43 aspect ratio, generous safe margins, no other text or watermark.

> Use case: logo-brand. Asset type: Steam transparent library logo. Text exactly "RomP", bold clean white sans serif lettering with very subtle cyan edge glow matching RomP app icon. Wide horizontal wordmark, generous transparent margins. Only the wordmark, no mascot, no background, no shadow panel, no watermark. Actual transparent background.

