use std::env;
use std::fs;
use std::num::NonZeroU32;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose;

const ESC: &str = "\x1b";
const ST: &str = "\x1b\\";
const KITTY_CHUNK_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MediaId(NonZeroU32);

impl MediaId {
    pub(crate) const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub(crate) const fn get(self) -> u32 {
        self.0.get()
    }
}

pub(crate) fn kitty_delete_image(image_id: MediaId) -> String {
    let image_id = image_id.get();
    wrap_for_tmux_if_needed(&format!("{ESC}_Ga=d,d=I,i={image_id},q=2;{ST}"))
}

pub(crate) fn kitty_transmit_png_with_id(
    path: &Path,
    columns: u16,
    rows: u16,
    image_id: Option<MediaId>,
) -> Result<String> {
    let png = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let payload = general_purpose::STANDARD.encode(png);
    let chunks = payload
        .as_bytes()
        .chunks(KITTY_CHUNK_SIZE)
        .collect::<Vec<_>>();

    let mut command = String::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk = std::str::from_utf8(chunk).context("base64 payload is not valid UTF-8")?;
        let has_more = index + 1 < chunks.len();
        let more_flag = u8::from(has_more);
        if index == 0 {
            let image_id = kitty_image_id_arg(image_id);
            command.push_str(&format!(
                "{ESC}_Ga=T,t=d,f=100,c={columns},r={rows},q=2{image_id},m={more_flag};{chunk}{ST}",
            ));
        } else {
            command.push_str(&format!("{ESC}_Gm={more_flag};{chunk}{ST}"));
        }
    }

    Ok(wrap_for_tmux_if_needed(&command))
}

pub(crate) fn kitty_transmit_png_file_with_id(
    path: &Path,
    columns: u16,
    rows: u16,
    image_id: Option<MediaId>,
) -> Result<String> {
    let path = path
        .canonicalize()
        .with_context(|| format!("canonicalize {}", path.display()))?;
    let payload = general_purpose::STANDARD.encode(path.to_string_lossy().as_bytes());
    let image_id = kitty_image_id_arg(image_id);
    let command = format!("{ESC}_Ga=T,t=f,f=100,c={columns},r={rows},q=2{image_id};{payload}{ST}");

    Ok(wrap_for_tmux_if_needed(&command))
}

fn kitty_image_id_arg(image_id: Option<MediaId>) -> String {
    image_id
        .map(|image_id| format!(",i={}", image_id.get()))
        .unwrap_or_default()
}

fn wrap_for_tmux_if_needed(command: &str) -> String {
    if env::var_os("TMUX").is_none() {
        return command.to_string();
    }

    let escaped = command.replace(ESC, "\x1b\x1b");
    format!("{ESC}Ptmux;{escaped}{ST}")
}
