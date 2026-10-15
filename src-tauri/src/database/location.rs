//! Refusing to keep the budget somewhere SQLite cannot keep it safely.
//!
//! SQLite relies on the operating system's file locks to stop two writers
//! colliding. On a network drive (SMB, NFS, AFP, WebDAV) those locks are often
//! approximate or missing, and a cloud-sync service (Dropbox, OneDrive, iCloud
//! Drive and the rest) copies the file — and its write-ahead log — from under
//! the program while it is open, then copies another computer's version back
//! over it. Either way is how budgets get corrupted, and the damage usually
//! shows up weeks later, far from the cause.
//!
//! So indiBudget refuses to open its database in such a place, and says why.
//! Sharing a budget between computers is what hosting is for: one computer
//! keeps the file on its own disk and the others connect to it.
//!
//! The database lives in the app's data folder, which is normally local. It
//! ends up somewhere unsafe when that folder is redirected: a Windows profile
//! redirected to a server, a Linux home directory on NFS, or a data folder
//! someone moved into Dropbox and left a link behind. Anyone in that position
//! can point indiBudget at a local folder with `INDIBUDGET_DATA_DIR`.

use std::path::{Component, Path, PathBuf};

/// Why a location is unsafe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Risk {
    /// On a network file system. `kind` names it as people know it.
    Network { kind: String },
    /// Inside a folder a sync service copies around.
    CloudSync { service: String },
}

/// The sentence shown instead of the budget.
pub fn refusal(path: &Path, risk: &Risk) -> String {
    let place = match risk {
        Risk::Network { kind } => format!("on a network drive ({kind})"),
        Risk::CloudSync { service } => format!("in a folder that {service} keeps in sync"),
    };
    format!(
        "indiBudget will not open your budget at {} because it is {place}. \
         The database can be corrupted when it is kept there: the network or the \
         sync service can copy it while indiBudget is writing to it. \
         To share one budget between computers, keep it on one computer's own disk \
         and connect the others from Sharing. To keep your budget somewhere else on \
         this computer, set INDIBUDGET_DATA_DIR to a folder on a local disk and start \
         indiBudget again.",
        path.display()
    )
}

/// Judge where a database file would live. `None` means it is safe.
///
/// The file need not exist yet: the nearest folder that does is judged, after
/// following links, so a data folder that is really a link into Dropbox is
/// caught.
pub fn assess(path: &Path) -> Option<Risk> {
    let real = nearest_existing(path)
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| path.to_path_buf());

    // Judged on both the path as given and the resolved one: the resolved one
    // finds links, the given one keeps Windows UNC paths recognisable.
    for candidate in [path, real.as_path()] {
        if let Some(risk) = network_by_name(candidate) {
            return Some(risk);
        }
        if let Some(service) = sync_service_by_name(candidate) {
            return Some(Risk::CloudSync { service });
        }
    }
    if let Some(service) = sync_service_by_marker(&real) {
        return Some(Risk::CloudSync { service });
    }
    if let Some(service) = sync_service_by_environment(&real) {
        return Some(Risk::CloudSync { service });
    }
    platform::network_file_system(&real).map(|kind| Risk::Network { kind })
}

fn nearest_existing(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.exists()).map(Path::to_path_buf)
}

fn components_lower(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

/// A Windows network path: `\\server\share\…` or `\\?\UNC\server\share\…`.
fn network_by_name(path: &Path) -> Option<Risk> {
    let text = path.to_string_lossy();
    let unc = (text.starts_with(r"\\") && !text.starts_with(r"\\?\") && !text.starts_with(r"\\.\"))
        || text.to_ascii_uppercase().starts_with(r"\\?\UNC\");
    unc.then(|| Risk::Network {
        kind: "a shared folder on another computer".into(),
    })
}

/// Folder names the sync services create, compared whole and ignoring case.
fn sync_service_by_name(path: &Path) -> Option<String> {
    for name in components_lower(path) {
        let service = match name.as_str() {
            "dropbox" => "Dropbox",
            n if n.starts_with("dropbox (") => "Dropbox",
            "onedrive" => "OneDrive",
            n if n.starts_with("onedrive - ") => "OneDrive",
            "google drive" | "googledrive" | "my drive" => "Google Drive",
            "icloud drive" | "icloud~drive" | "mobile documents" | "com~apple~clouddocs" => "iCloud Drive",
            // macOS keeps every File Provider service here: Dropbox, OneDrive,
            // Google Drive, Box and more.
            "cloudstorage" => "a cloud storage service",
            "box" | "box sync" => "Box",
            "pcloud drive" | "pclouddrive" => "pCloud",
            "nextcloud" => "Nextcloud",
            "owncloud" => "ownCloud",
            "synologydrive" | "synology drive" => "Synology Drive",
            "mega" | "megasync" => "MEGA",
            "tresorit" => "Tresorit",
            "sync.com" => "Sync.com",
            _ => continue,
        };
        return Some(service.to_string());
    }
    None
}

/// Files the sync services leave at the top of the folders they manage.
fn sync_service_by_marker(path: &Path) -> Option<String> {
    for folder in path.ancestors() {
        for (marker, service) in [
            (".dropbox", "Dropbox"),
            (".dropbox.cache", "Dropbox"),
            (".stfolder", "Syncthing"),
            (".sync", "Resilio Sync"),
            (".tresorit", "Tresorit"),
        ] {
            if folder.join(marker).exists() {
                return Some(service.to_string());
            }
        }
        // Nextcloud and ownCloud keep a journal named `.sync_<id>.db` or
        // `._sync_<id>.db` in the root of the synced folder.
        if let Ok(entries) = std::fs::read_dir(folder) {
            let journal = entries.flatten().any(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                (name.starts_with(".sync_") || name.starts_with("._sync_")) && name.ends_with(".db")
            });
            if journal {
                return Some("Nextcloud".into());
            }
        }
    }
    None
}

/// OneDrive says where it syncs through environment variables on Windows,
/// whatever the folder is called.
fn sync_service_by_environment(path: &Path) -> Option<String> {
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        let Some(root) = std::env::var_os(var).filter(|v| !v.is_empty()) else {
            continue;
        };
        let root = PathBuf::from(root);
        let root = root.canonicalize().unwrap_or(root);
        if path.starts_with(&root) {
            return Some("OneDrive".into());
        }
    }
    None
}

/// File-system types that live on another computer, as Linux names them.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_network_kind(fs_type: &str) -> Option<&'static str> {
    let kind = match fs_type {
        "nfs" | "nfs4" => "NFS",
        "cifs" | "smb3" | "smbfs" => "SMB",
        "afs" | "ncpfs" | "coda" => "a network file system",
        "ceph" | "glusterfs" | "lustre" | "beegfs" | "gpfs" => "a cluster file system",
        "davfs" | "fuse.davfs" | "fuse.davfs2" => "WebDAV",
        "fuse.sshfs" => "SSHFS",
        "fuse.rclone" => "rclone",
        "fuse.s3fs" | "fuse.goofys" | "fuse.gcsfuse" => "cloud storage",
        "fuse.gvfsd-fuse" => "a network location mounted by GNOME",
        "fuse.kio-fuse" => "a network location mounted by KDE",
        _ => return None,
    };
    Some(kind)
}

/// Undo the octal escapes `/proc/self/mountinfo` uses for spaces and the like.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn unescape_mount_path(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // `\` followed by exactly three octal digits, e.g. `\040` for a space.
        if bytes[i] == b'\\' {
            if let Some(digits) = bytes.get(i + 1..i + 4) {
                if digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                    let code = digits.iter().fold(0u32, |n, d| n * 8 + u32::from(d - b'0'));
                    if let Ok(code) = u8::try_from(code) {
                        out.push(code);
                        i += 4;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The file-system type of the mount `path` is on, from the text of
/// `/proc/self/mountinfo`. The deepest mount point that contains it wins.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn mount_type_for(path: &Path, mountinfo: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let Some((before, after)) = line.split_once(" - ") else {
            continue;
        };
        let Some(mount_point) = before.split(' ').nth(4) else {
            continue;
        };
        let Some(fs_type) = after.split(' ').next() else {
            continue;
        };
        let mount_point = PathBuf::from(unescape_mount_path(mount_point));
        if path.starts_with(&mount_point) {
            let depth = mount_point.components().count();
            // Later lines are mounted over earlier ones at the same point.
            if best.as_ref().is_none_or(|(d, _)| depth >= *d) {
                best = Some((depth, fs_type.to_string()));
            }
        }
    }
    best.map(|(_, t)| t)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::Path;

    pub fn network_file_system(path: &Path) -> Option<String> {
        let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
        let fs_type = super::mount_type_for(path, &mountinfo)?;
        super::linux_network_kind(&fs_type).map(str::to_string)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// macOS marks every local volume `MNT_LOCAL`; anything without it is on
    /// another computer, whatever protocol it speaks.
    pub fn network_file_system(path: &Path) -> Option<String> {
        let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c_path.as_ptr(), &mut stats) } != 0 {
            return None;
        }
        if stats.f_flags & (libc::MNT_LOCAL as u32) != 0 {
            return None;
        }
        let name = unsafe { CStr::from_ptr(stats.f_fstypename.as_ptr()) }
            .to_string_lossy()
            .to_lowercase();
        Some(
            match name.as_str() {
                "smbfs" => "SMB",
                "nfs" => "NFS",
                "afpfs" => "AFP",
                "webdav" => "WebDAV",
                _ => "a network file system",
            }
            .to_string(),
        )
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Path, Prefix};

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    }
    const DRIVE_REMOTE: u32 = 4;

    /// A drive letter mapped to a shared folder on another computer.
    pub fn network_file_system(path: &Path) -> Option<String> {
        let letter = match path.components().next()? {
            Component::Prefix(p) => match p.kind() {
                Prefix::Disk(l) | Prefix::VerbatimDisk(l) => l as char,
                _ => return None,
            },
            _ => return None,
        };
        let root: Vec<u16> = std::ffi::OsStr::new(&format!("{letter}:\\"))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        (unsafe { GetDriveTypeW(root.as_ptr()) } == DRIVE_REMOTE)
            .then(|| format!("mapped drive {letter}:"))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn network_file_system(_path: &std::path::Path) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("indibudget-location-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_usual_data_folder_is_fine() {
        let dir = scratch("plain");
        assert_eq!(assess(&dir.join("indibudget").join("indibudget.db")), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_folders_are_recognised_by_name() {
        for (path, service) in [
            ("/home/sam/Dropbox/indibudget/indibudget.db", "Dropbox"),
            ("/Users/sam/Dropbox (Personal)/indibudget.db", "Dropbox"),
            ("/Users/sam/Library/CloudStorage/GoogleDrive-sam@example.com/My Drive/x.db", "a cloud storage service"),
            ("/Users/sam/Library/Mobile Documents/com~apple~CloudDocs/x.db", "iCloud Drive"),
            ("/home/sam/Nextcloud/x.db", "Nextcloud"),
        ] {
            assert_eq!(sync_service_by_name(Path::new(path)).as_deref(), Some(service), "{path}");
        }
        assert_eq!(
            sync_service_by_name(Path::new("C:/Users/Sam/OneDrive - Contoso/indibudget.db")).as_deref(),
            Some("OneDrive")
        );
    }

    #[test]
    fn similar_names_are_not_mistaken_for_sync_folders() {
        for path in [
            "/home/sam/.local/share/indibudget/indibudget.db",
            "/home/sam/dropbox-notes/indibudget.db",
            "/home/sam/my-onedrive-export/indibudget.db",
            "/home/sam/boxes/indibudget.db",
        ] {
            assert_eq!(sync_service_by_name(Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn a_synced_folder_is_found_by_the_marker_its_service_leaves() {
        let dir = scratch("marker");
        std::fs::create_dir_all(dir.join("budgets")).unwrap();
        std::fs::write(dir.join(".stfolder"), "").unwrap();
        assert_eq!(
            assess(&dir.join("budgets").join("indibudget.db")),
            Some(Risk::CloudSync { service: "Syncthing".into() })
        );

        let nc = scratch("nextcloud-journal");
        std::fs::write(nc.join(".sync_4f2a9c.db"), "").unwrap();
        assert_eq!(sync_service_by_marker(&nc).as_deref(), Some("Nextcloud"));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(nc);
    }

    #[cfg(unix)]
    #[test]
    fn a_data_folder_that_is_a_link_into_a_synced_folder_is_caught() {
        let dir = scratch("link");
        let synced = dir.join("synced");
        std::fs::create_dir_all(&synced).unwrap();
        std::fs::create_dir(synced.join(".dropbox")).unwrap();
        let data = dir.join("data");
        std::os::unix::fs::symlink(&synced, &data).unwrap();
        assert_eq!(
            assess(&data.join("indibudget.db")),
            Some(Risk::CloudSync { service: "Dropbox".into() })
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn windows_shared_folders_are_recognised() {
        assert!(network_by_name(Path::new(r"\\fileserver\profiles\sam\AppData\Roaming\indibudget\indibudget.db")).is_some());
        assert!(network_by_name(Path::new(r"\\?\UNC\fileserver\profiles\indibudget.db")).is_some());
        assert!(network_by_name(Path::new(r"\\?\C:\Users\Sam\indibudget.db")).is_none());
        assert!(network_by_name(Path::new(r"C:\Users\Sam\AppData\Roaming\indibudget.db")).is_none());
    }

    const MOUNTINFO: &str = "\
22 1 8:2 / / rw,relatime - ext4 /dev/sda2 rw
40 22 0:45 / /home rw,relatime - nfs4 nas:/export/home rw,vers=4.2
41 40 8:3 / /home/sam/local\\040disk rw,relatime - ext4 /dev/sdb1 rw
42 22 0:50 / /mnt/share rw,relatime - cifs //nas/share rw
43 22 0:51 / /run/user/1000/gvfs rw - fuse.gvfsd-fuse gvfsd-fuse rw
";

    #[test]
    fn the_deepest_mount_decides_the_file_system() {
        let at = |p: &str| mount_type_for(Path::new(p), MOUNTINFO);
        assert_eq!(at("/home/sam/.local/share/indibudget/indibudget.db").as_deref(), Some("nfs4"));
        assert_eq!(at("/home/sam/local disk/indibudget.db").as_deref(), Some("ext4"), "escaped space");
        assert_eq!(at("/mnt/share/x.db").as_deref(), Some("cifs"));
        assert_eq!(at("/var/lib/x.db").as_deref(), Some("ext4"));
        // `/home/sammy` is not under `/home/sam/local disk`, only under `/home`.
        assert_eq!(at("/home/sammy/x.db").as_deref(), Some("nfs4"));
    }

    #[test]
    fn network_file_systems_are_named_and_local_ones_are_not() {
        assert_eq!(linux_network_kind("nfs4"), Some("NFS"));
        assert_eq!(linux_network_kind("cifs"), Some("SMB"));
        assert_eq!(linux_network_kind("fuse.sshfs"), Some("SSHFS"));
        for local in ["ext4", "btrfs", "xfs", "zfs", "tmpfs", "overlay", "fuseblk", "fuse.ntfs-3g", "vfat"] {
            assert_eq!(linux_network_kind(local), None, "{local}");
        }
    }

    #[test]
    fn the_refusal_says_where_why_and_what_to_do() {
        let sentence = refusal(
            Path::new("/home/sam/Dropbox/indibudget/indibudget.db"),
            &Risk::CloudSync { service: "Dropbox".into() },
        );
        assert!(sentence.contains("/home/sam/Dropbox/indibudget/indibudget.db"));
        assert!(sentence.contains("Dropbox keeps in sync"));
        assert!(sentence.contains("Sharing"));
        assert!(sentence.contains("INDIBUDGET_DATA_DIR"));
    }
}
