use std::path::{Path, PathBuf};

use super::OnDisk;

/// The folder the last export was written to, and the document it was written from.
pub(crate) struct ExportFolder {
    folder: PathBuf,
    /// `OnDisk::project_path` at the time of the export.
    project: Option<String>,
}

/// THE FOLDER AN EXPORT CHOOSER OPENS IN: the last export's folder of this document, else the document's own.
pub(crate) fn export_folder(disk: &OnDisk) -> Option<PathBuf> {
    let remembered = disk.export_folder.as_ref().filter(|e| e.project == disk.project_path).map(|e| e.folder.clone());
    remembered.or_else(|| project_folder(disk.project_path.as_deref()))
}

/// REMEMBER THE FOLDER OF `written`, the file an export was just written to, for the next export of this document.
pub(crate) fn remember_export(disk: &mut OnDisk, written: &Path) {
    if let Some(folder) = written.parent().filter(|f| !f.as_os_str().is_empty()) {
        disk.export_folder = Some(ExportFolder { folder: folder.to_path_buf(), project: disk.project_path.clone() });
    }
}

fn project_folder(project_path: Option<&str>) -> Option<PathBuf> {
    Path::new(project_path?).parent().filter(|f| !f.as_os_str().is_empty()).map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_at(path: Option<&str>) -> OnDisk {
        OnDisk { project_path: path.map(str::to_string), ..OnDisk::default() }
    }

    #[test]
    fn a_saved_project_offers_its_own_folder() {
        assert_eq!(export_folder(&saved_at(Some("/work/plate/plate.qcad"))), Some(PathBuf::from("/work/plate")));
    }

    #[test]
    fn a_project_never_saved_offers_no_folder() {
        assert_eq!(export_folder(&saved_at(None)), None);
    }

    #[test]
    fn a_bare_file_name_offers_no_folder() {
        assert_eq!(export_folder(&saved_at(Some("plate.qcad"))), None);
    }

    #[test]
    fn the_folder_of_the_last_export_comes_first() {
        let mut disk = saved_at(Some("/work/plate/plate.qcad"));
        remember_export(&mut disk, Path::new("/out/step/plate.step"));
        assert_eq!(export_folder(&disk), Some(PathBuf::from("/out/step")));
    }

    #[test]
    fn the_folder_of_the_last_export_is_not_offered_to_another_document() {
        let mut disk = saved_at(Some("/work/plate/plate.qcad"));
        remember_export(&mut disk, Path::new("/out/step/plate.step"));
        disk.project_path = Some("/work/lid/lid.qcad".into());
        assert_eq!(export_folder(&disk), Some(PathBuf::from("/work/lid")));
    }
}
