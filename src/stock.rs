//! System stock icon extraction via SHGetStockIconInfo

use crate::error::IconError;
use crate::extract::{AutoIcon, IconData};
use windows::Win32::UI::Shell::*;

/// System-defined stock icon identifiers.
#[derive(Clone, Copy, Debug)]
pub enum StockIcon {
    DocNoAssoc,
    DocAssoc,
    Application,
    Folder,
    FolderOpen,
    DriveRemovable,
    DriveFixed,
    DriveNet,
    DriveNetDisabled,
    DriveCd,
    DriveRam,
    World,
    Server,
    Printer,
    MyNetwork,
    Find,
    Help,
    Share,
    Link,
    Recycler,
    RecyclerFull,
    Lock,
    DriveUnknown,
    DriveDvd,
    Shield,
    Warning,
    Info,
    Error,
    Key,
    Software,
    DesktopPc,
    MobilePc,
    Users,
    Internet,
    ZipFile,
    Settings,
    /// Escape hatch for any SHSTOCKICONID value not listed above.
    Custom(i32),
}

impl StockIcon {
    fn as_i32(self) -> i32 {
        match self {
            Self::DocNoAssoc => 0, Self::DocAssoc => 1, Self::Application => 2,
            Self::Folder => 3, Self::FolderOpen => 4, Self::DriveRemovable => 7,
            Self::DriveFixed => 8, Self::DriveNet => 9, Self::DriveNetDisabled => 10,
            Self::DriveCd => 11, Self::DriveRam => 12, Self::World => 13,
            Self::Server => 15, Self::Printer => 16, Self::MyNetwork => 17,
            Self::Find => 22, Self::Help => 23, Self::Share => 28, Self::Link => 29,
            Self::Recycler => 31, Self::RecyclerFull => 32, Self::Lock => 47,
            Self::DriveUnknown => 58, Self::DriveDvd => 59, Self::Shield => 77,
            Self::Warning => 78, Self::Info => 79, Self::Error => 80, Self::Key => 81,
            Self::Software => 82, Self::DesktopPc => 94, Self::MobilePc => 95,
            Self::Users => 96, Self::Internet => 104, Self::ZipFile => 105,
            Self::Settings => 106, Self::Custom(v) => v,
        }
    }
}

/// Extract a system stock icon by ID.
pub fn extract_stock_icon(icon: StockIcon) -> Result<IconData, IconError> {
    extract_stock_icon_sized(icon, 0)
}

/// Extract a system stock icon at a specific size (0 = system default).
pub fn extract_stock_icon_sized(icon: StockIcon, size: i32) -> Result<IconData, IconError> {
    unsafe {
        let flags = SHGSI_ICON | SHGSI_LARGEICON;
        let mut sii = SHSTOCKICONINFO {
            cbSize: std::mem::size_of::<SHSTOCKICONINFO>() as u32,
            ..Default::default()
        };
        let siid = SHSTOCKICONID(icon.as_i32());
        SHGetStockIconInfo(siid, flags, &mut sii)
            .map_err(|e| IconError::Extract(format!("SHGetStockIconInfo: {e}")))?;

        let _guard = AutoIcon(sii.hIcon);
        let (w, h) = if size > 0 {
            (size, size)
        } else {
            crate::extract::system_icon_size()
        };

        crate::extract::hicon_to_rgba(sii.hIcon, w, h)
            .ok_or_else(|| IconError::Extract("stock icon conversion failed".into()))
    }
}
