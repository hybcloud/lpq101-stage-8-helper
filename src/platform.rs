use std::{ptr::copy_nonoverlapping, thread, time::Duration};

use anyhow::{Context as _, Result};
use windows::Win32::{
    Foundation::{GlobalFree, HANDLE, HWND},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
    UI::Input::KeyboardAndMouse::{
        MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_NEXT, VK_PRIOR,
    },
};

pub const HOTKEY_PREVIOUS: i32 = 0x4a4d_5301;
pub const HOTKEY_NEXT: i32 = 0x4a4d_5302;
const CF_UNICODETEXT: u32 = 13;

pub fn copy_to_clipboard(owner: Option<HWND>, text: &str) -> Result<()> {
    let mut wide = text.encode_utf16().collect::<Vec<_>>();
    wide.push(0);

    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(owner).is_ok() {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(8));
        }
        anyhow::ensure!(opened, "the clipboard is busy");

        let result = (|| -> Result<()> {
            EmptyClipboard().context("clear clipboard")?;
            let allocation = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>())
                .context("allocate clipboard memory")?;
            let destination = GlobalLock(allocation).cast::<u16>();
            if destination.is_null() {
                let _ = GlobalFree(Some(allocation));
                anyhow::bail!("lock clipboard memory");
            }
            copy_nonoverlapping(wide.as_ptr(), destination, wide.len());
            // GlobalUnlock returns zero when the final lock is released, so its
            // generated Result cannot reliably distinguish that success case.
            let _ = GlobalUnlock(allocation);

            if let Err(error) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(allocation.0))) {
                let _ = GlobalFree(Some(allocation));
                return Err(error).context("set clipboard text");
            }
            // SetClipboardData owns the allocation after a successful call.
            Ok(())
        })();

        let close_result = CloseClipboard().context("close clipboard");
        result.and(close_result)
    }
}

pub struct GlobalHotkeys {
    owner: Option<HWND>,
    registered: bool,
}

impl GlobalHotkeys {
    pub fn register(owner: Option<HWND>, enabled: bool) -> Self {
        if !enabled {
            return Self {
                owner,
                registered: false,
            };
        }

        let previous = unsafe {
            RegisterHotKey(owner, HOTKEY_PREVIOUS, MOD_NOREPEAT, VK_PRIOR.0 as u32).is_ok()
        };
        let next =
            unsafe { RegisterHotKey(owner, HOTKEY_NEXT, MOD_NOREPEAT, VK_NEXT.0 as u32).is_ok() };
        let registered = previous && next;
        if !registered {
            unsafe {
                if previous {
                    let _ = UnregisterHotKey(owner, HOTKEY_PREVIOUS);
                }
                if next {
                    let _ = UnregisterHotKey(owner, HOTKEY_NEXT);
                }
            }
        }
        Self { owner, registered }
    }

    pub fn is_registered(&self) -> bool {
        self.registered
    }
}

impl Drop for GlobalHotkeys {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                let _ = UnregisterHotKey(self.owner, HOTKEY_PREVIOUS);
                let _ = UnregisterHotKey(self.owner, HOTKEY_NEXT);
            }
        }
    }
}
