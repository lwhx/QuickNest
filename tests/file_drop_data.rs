#![cfg(windows)]
//! Synthetic Windows IDataObject tests; these do not exercise Explorer drag-and-drop.

use quicknest::infrastructure::windows::file_drop::{FileDropStats, read_files, supports_files};
use std::{mem::size_of, path::PathBuf, ptr};
use windows::{
    Win32::{
        Foundation::{DV_E_FORMATETC, E_FAIL, E_NOTIMPL, GlobalFree, S_OK},
        System::{
            Com::{
                DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl,
                IEnumFORMATETC, IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
            },
            Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::Shell::DROPFILES,
    },
    core::{BOOL, HRESULT, Ref, Result, implement},
};

const CF_HDROP: u16 = 15; // Avoid requiring the DataExchange feature just for this constant.

enum Reply {
    Files(Vec<String>),
    Unsupported,
    WrongTymed,
    NullGlobal,
    EmptyFiles,
    GetDataFailure,
}

#[implement(IDataObject, Agile = false)]
struct SyntheticDataObject {
    reply: Reply,
}

fn object(reply: Reply) -> IDataObject {
    SyntheticDataObject { reply }.into()
}

fn format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP,
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
        ..Default::default()
    }
}

fn matches_format(raw: *const FORMATETC) -> bool {
    // SAFETY: COM callers provide a readable FORMATETC or null; null is rejected.
    let Some(f) = (unsafe { raw.as_ref() }) else {
        return false;
    };
    f.cfFormat == CF_HDROP
        && f.ptd.is_null()
        && f.dwAspect == DVASPECT_CONTENT.0
        && f.lindex == -1
        && f.tymed == TYMED_HGLOBAL.0 as u32
}

fn drop_medium(paths: &[String]) -> Result<STGMEDIUM> {
    let mut wide: Vec<u16> = paths
        .iter()
        .flat_map(|p| p.encode_utf16().chain(std::iter::once(0)))
        .collect();
    wide.push(0);
    if paths.is_empty() {
        wide.push(0);
    } // Empty lists also have a double terminator.
    let header_len = size_of::<DROPFILES>();
    let bytes = header_len + wide.len() * size_of::<u16>();
    // SAFETY: The nonzero allocation size covers the header and complete UTF-16 list.
    let global = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes)? };
    // SAFETY: global is a live, newly allocated movable HGLOBAL owned by this function.
    let data = unsafe { GlobalLock(global) }.cast::<u8>();
    if data.is_null() {
        // SAFETY: Locking failed, so this owned allocation has no outstanding lock.
        let _ = unsafe { GlobalFree(Some(global)) };
        return Err(E_FAIL.into());
    }
    let header = DROPFILES {
        pFiles: header_len as u32,
        fWide: BOOL(1),
        ..Default::default()
    };
    // SAFETY: data is locked for bytes bytes; both writes fit and sources do not overlap.
    unsafe {
        data.cast::<DROPFILES>().write_unaligned(header);
        ptr::copy_nonoverlapping(
            wide.as_ptr().cast::<u8>(),
            data.add(header_len),
            wide.len() * size_of::<u16>(),
        );
    }
    // SAFETY: This balances the successful lock; zero can mean the last lock was released.
    let _ = unsafe { GlobalUnlock(global) };
    // Ownership transfers to the caller, which must call ReleaseStgMedium exactly once.
    Ok(STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 { hGlobal: global },
        ..Default::default()
    })
}

impl IDataObject_Impl for SyntheticDataObject_Impl {
    fn GetData(&self, f: *const FORMATETC) -> Result<STGMEDIUM> {
        if self.QueryGetData(f) != S_OK {
            return Err(DV_E_FORMATETC.into());
        }
        match &self.reply {
            Reply::Files(paths) => drop_medium(paths),
            Reply::EmptyFiles => drop_medium(&[]),
            // TYMED_NULL has no resource; never tag fake pointers as stream/GDI handles.
            Reply::WrongTymed => Ok(STGMEDIUM::default()),
            // A null HGLOBAL contains no allocation for ReleaseStgMedium to free.
            Reply::NullGlobal => Ok(STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                ..Default::default()
            }),
            Reply::GetDataFailure => Err(E_FAIL.into()),
            Reply::Unsupported => Err(DV_E_FORMATETC.into()),
        }
    }
    fn QueryGetData(&self, f: *const FORMATETC) -> HRESULT {
        if !matches!(self.reply, Reply::Unsupported) && matches_format(f) {
            S_OK
        } else {
            DV_E_FORMATETC
        }
    }
    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
        E_NOTIMPL
    }
    fn SetData(&self, _: *const FORMATETC, _: *const STGMEDIUM, _: BOOL) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn EnumFormatEtc(&self, _: u32) -> Result<IEnumFORMATETC> {
        Err(E_NOTIMPL.into())
    }
    fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<IAdviseSink>) -> Result<u32> {
        Err(E_NOTIMPL.into())
    }
    fn DUnadvise(&self, _: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(E_NOTIMPL.into())
    }
}

fn stats() -> FileDropStats {
    FileDropStats::default()
}
fn counts(stats: &FileDropStats) -> (u32, u32) {
    (stats.acquired.get(), stats.released.get())
}

#[test]
fn unicode_spaces_multiple_paths_link_and_folder_preserve_order_and_values() {
    // No filesystem setup: these names test transport, not link resolution or folder discovery.
    let paths = [
        r"C:\导入 空间\报告一.txt",
        r"D:\data set\second file.txt",
        r"C:\导入 空间\快捷方式.LNK",
        r"E:\目录 有空格\子文件夹\",
    ];
    let data = object(Reply::Files(
        paths.iter().map(|p| (*p).to_owned()).collect(),
    ));
    let stats = stats();
    assert!(supports_files(&data));
    assert_eq!(counts(&stats), (0, 0));
    let actual = read_files(&data, &stats).expect("valid synthetic CF_HDROP");
    let expected: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    // Compare raw strings too: Path equality can normalize separators/components.
    assert_eq!(
        actual.iter().map(|p| p.as_os_str()).collect::<Vec<_>>(),
        expected.iter().map(|p| p.as_os_str()).collect::<Vec<_>>()
    );
    assert_eq!(counts(&stats), (1, 1));
}

#[test]
fn unsupported_format_does_not_acquire_a_medium() {
    let data = object(Reply::Unsupported);
    let stats = stats();
    assert!(!supports_files(&data));
    assert!(read_files(&data, &stats).is_err());
    assert_eq!(counts(&stats), (0, 0));
}

#[test]
fn invalid_successful_media_are_released_once() {
    for reply in [Reply::WrongTymed, Reply::NullGlobal, Reply::EmptyFiles] {
        let data = object(reply);
        let stats = stats();
        assert!(supports_files(&data));
        assert!(read_files(&data, &stats).is_err());
        assert_eq!(counts(&stats), (1, 1));
    }
}

#[test]
fn failed_get_data_does_not_acquire_a_medium() {
    let data = object(Reply::GetDataFailure);
    let stats = stats();
    assert!(supports_files(&data));
    assert!(read_files(&data, &stats).is_err());
    assert_eq!(counts(&stats), (0, 0));
}

#[test]
fn closed_ui_queue_rejects_copy_and_releases_the_successful_medium() {
    use quicknest::infrastructure::windows::file_drop::receiving_target;
    use std::{rc::Rc, sync::mpsc};
    use windows::Win32::{
        Foundation::POINTL,
        System::{
            Ole::{DROPEFFECT_COPY, DROPEFFECT_NONE},
            SystemServices::MODIFIERKEYS_FLAGS,
        },
    };
    let (sender, receiver) = mpsc::sync_channel(4);
    drop(receiver);
    let stats = Rc::new(FileDropStats::default());
    let target = receiving_target(sender, stats.clone());
    let data = object(Reply::Files(vec![r"C:\中文 空间\input.txt".into()]));
    let mut effect = DROPEFFECT_COPY;
    // SAFETY: Same-thread live COM objects, valid point/effect storage. Real CF_HDROP
    // data is copied then must be rejected when its UI event cannot be submitted.
    let result = unsafe {
        target.interface().Drop(
            &data,
            MODIFIERKEYS_FLAGS(0),
            POINTL { x: 1, y: 2 },
            &mut effect,
        )
    };
    assert!(result.is_err());
    assert_eq!(effect, DROPEFFECT_NONE);
    assert_eq!(counts(&stats), (1, 1));
    assert_eq!(stats.errors.get(), 1);
    drop(target);
    assert_eq!(stats.destroyed.get(), 1);
}

#[test]
fn synthetic_query_get_data_requires_the_exact_format() {
    let data = object(Reply::EmptyFiles);
    let good = format();
    // SAFETY: good is a live FORMATETC, and this same-thread synthetic COM object is live.
    assert_eq!(unsafe { data.QueryGetData(&good) }, S_OK);
    let mut variants = [format(); 5];
    variants[0].cfFormat = 1;
    variants[1].dwAspect = 0;
    variants[2].lindex = 0;
    variants[3].tymed = 0;
    variants[4].tymed |= 2; // A mask containing HGLOBAL still fails strict matching.
    for bad in variants {
        // SAFETY: bad is a live FORMATETC; the implementation only reads its fields.
        assert_eq!(unsafe { data.QueryGetData(&bad) }, DV_E_FORMATETC);
    }
}
