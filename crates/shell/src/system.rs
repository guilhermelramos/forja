//! Windows theme and language. Other targets fall back to dark and pt-BR.

use forja_kernel::Locale;

pub fn system_is_dark() -> bool {
    #[cfg(windows)]
    {
        if let Some(light) = apps_use_light_theme() {
            return !light;
        }
        window_color_is_dark()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

pub fn system_reduce_motion() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
        };
        let mut enabled = 1u32;
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut enabled as *mut u32).cast(),
                0,
            )
        };
        ok != 0 && enabled == 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn system_locale() -> Locale {
    #[cfg(windows)]
    {
        let language = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        let primary = language & 0x03FF;
        if primary == 0x09 {
            Locale::En
        } else {
            Locale::PtBr
        }
    }
    #[cfg(not(windows))]
    {
        Locale::PtBr
    }
}

#[cfg(windows)]
fn apps_use_light_theme() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = wide("AppsUseLightTheme");
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    if status == 0 {
        Some(data != 0)
    } else {
        None
    }
}

#[cfg(windows)]
fn window_color_is_dark() -> bool {
    use windows_sys::Win32::Graphics::Gdi::{GetSysColor, COLOR_WINDOW};
    let color = unsafe { GetSysColor(COLOR_WINDOW) };
    let red = (color & 0xFF) as f32;
    let green = ((color >> 8) & 0xFF) as f32;
    let blue = ((color >> 16) & 0xFF) as f32;
    (0.2126 * red) + (0.7152 * green) + (0.0722 * blue) < 128.0
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
