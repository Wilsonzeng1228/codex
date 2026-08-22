use std::env;
use std::ffi::OsString;
use std::fs;

use serial_test::serial;

use super::MediaId;
use super::iterm2_transmit_png;
use super::kitty_delete_image;
use super::kitty_transmit_png_with_id;

struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn remove(name: &'static str) -> Self {
        let previous = env::var_os(name);
        unsafe { env::remove_var(name) };
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { env::set_var(self.name, value) },
            None => unsafe { env::remove_var(self.name) },
        }
    }
}

#[test]
#[serial]
fn kitty_commands_are_scoped_to_the_supplied_media_id() {
    let _tmux = EnvVarGuard::remove("TMUX");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    fs::write(&path, b"png").unwrap();

    let media_id = MediaId::new(23).expect("non-zero media id");
    let transmit =
        kitty_transmit_png_with_id(&path, /*columns*/ 10, /*rows*/ 4, Some(media_id)).unwrap();

    assert!(transmit.contains("c=10,r=4,q=2,i=23,m=0;"));
    assert_eq!(
        kitty_delete_image(media_id),
        "\x1b_Ga=d,d=I,i=23,q=2;\x1b\\"
    );
    assert_eq!(media_id.get(), 23);
    assert_eq!(MediaId::new(0), None);
}

#[test]
fn iterm2_inline_command_uses_cell_dimensions_and_allows_terminal_cursor_movement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    fs::write(&path, b"png").unwrap();

    let command = iterm2_transmit_png(&path, /*columns*/ 10, /*rows*/ 4).unwrap();

    assert_eq!(
        command,
        "\x1b]1337;File=size=3;width=10;height=4;inline=1:cG5n\x1b\\"
    );
}
