// Remove this allow once main.rs or ui.rs calls into this module.
#![allow(dead_code)]

use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::annotation::{Annotation, AnnotationError};

pub const FORMAT_VERSION: &str = "1.0";
const ALLOWED_FPS: [u32; 5] = [5, 10, 12, 15, 20];

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Validation(String),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "file error: {e}"),
            Self::Json(e) => write!(f, "invalid project file: {e}"),
            Self::Validation(msg) => write!(f, "invalid project file: {msg}"),
        }
    }
}

impl std::error::Error for ProjectError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub version: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    #[serde(default)]
    pub trim_start_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_end_ms: Option<u64>,
    pub annotations: Vec<Annotation>,
}

impl Project {
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.version != FORMAT_VERSION {
            return Err(ProjectError::Validation(format!(
                "unsupported version {:?}, expected {FORMAT_VERSION:?}",
                self.version
            )));
        }
        if self.width == 0 || self.height == 0 {
            return Err(ProjectError::Validation(
                "width and height must be greater than 0".to_string(),
            ));
        }
        if !ALLOWED_FPS.contains(&self.fps) {
            return Err(ProjectError::Validation(format!(
                "fps {} is not one of {ALLOWED_FPS:?}",
                self.fps
            )));
        }
        if let Some(end) = self.trim_end_ms {
            if end <= self.trim_start_ms {
                return Err(ProjectError::Validation(format!(
                    "trim_end_ms {end} must be greater than trim_start_ms {}",
                    self.trim_start_ms
                )));
            }
        }
        for (i, ann) in self.annotations.iter().enumerate() {
            ann.validate().map_err(|e| match e {
                AnnotationError::InvalidTimeRange { start_ms, end_ms } => ProjectError::Validation(
                    format!("annotation {i} has end_ms {end_ms} before start_ms {start_ms}"),
                ),
                other => ProjectError::Validation(format!("annotation {i}: {other}")),
            })?;
            let (x, y) = ann.position();
            if x >= self.width || y >= self.height {
                return Err(ProjectError::Validation(format!(
                    "annotation {i} at ({x}, {y}) is outside recording bounds {}x{}",
                    self.width, self.height
                )));
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, ProjectError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(ProjectError::Json)
    }

    pub fn from_json(json: &str) -> Result<Project, ProjectError> {
        let project: Project = serde_json::from_str(json).map_err(ProjectError::Json)?;
        project.validate()?;
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        fs::write(path, self.to_json()?).map_err(ProjectError::Io)
    }

    pub fn load(path: &Path) -> Result<Project, ProjectError> {
        Self::from_json(&fs::read_to_string(path).map_err(ProjectError::Io)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1_0_FILE: &str = r##"{
        "version": "1.0",
        "width": 800,
        "height": 600,
        "fps": 12,
        "trim_start_ms": 500,
        "trim_end_ms": 4500,
        "annotations": [
            {"type": "arrow", "x": 120, "y": 80, "width": 200, "height": 40, "color": "#FF0000", "thickness": 3, "start_ms": 1000, "end_ms": 3000},
            {"type": "text", "x": 100, "y": 200, "text": "Clicking here does nothing", "color": "#FFFFFF", "background": "#FF0000", "font_size": 14, "start_ms": 1000, "end_ms": 3000},
            {"type": "blur", "x": 500, "y": 10, "width": 200, "height": 30, "radius": 12, "start_ms": 0, "end_ms": 4500}
        ]
    }"##;

    fn sample_project() -> Project {
        Project::from_json(V1_0_FILE).expect("fixture should parse")
    }

    #[test]
    fn v1_file_still_loads() {
        let p = sample_project();
        assert_eq!(p.width, 800);
        assert_eq!(p.fps, 12);
        assert_eq!(p.trim_start_ms, 500);
        assert_eq!(p.trim_end_ms, Some(4500));
        assert_eq!(p.annotations.len(), 3);
        // The fixture still uses the v1 width/height arrow form; the
        // loader migrates it to tip coordinates (120+200, 80+40).
        match &p.annotations[0] {
            Annotation::Arrow(a) => assert_eq!((a.tip_x, a.tip_y), (320, 120)),
            other => panic!("expected arrow, got {other:?}"),
        }
    }

    #[test]
    fn arrow_tip_pair_format_loads() {
        let json = r##"{"version":"1.0","width":800,"height":600,"fps":12,
            "annotations":[{"type":"arrow","x":60,"y":50,"tip_x":10,"tip_y":20,
            "color":"#FF0000","thickness":3,"start_ms":0,"end_ms":1000}]}"##;
        let p = Project::from_json(json).expect("tip_x/tip_y form loads");
        match &p.annotations[0] {
            // Tip can sit up-left of the tail — any direction now.
            Annotation::Arrow(a) => assert_eq!((a.x, a.y, a.tip_x, a.tip_y), (60, 50, 10, 20)),
            other => panic!("expected arrow, got {other:?}"),
        }
    }

    #[test]
    fn json_round_trip_preserves_project() {
        let project = sample_project();
        let json = project.to_json().expect("serialize");
        let back = Project::from_json(&json).expect("round trip load");
        assert_eq!(project, back);
    }

    #[test]
    fn file_save_and_load_round_trip() {
        let project = sample_project();
        let path =
            std::env::temp_dir().join(format!("bloop-test-{}.bloop.json", std::process::id()));
        project.save(&path).expect("save file");
        let loaded = Project::load(&path).expect("load file");
        let _ = fs::remove_file(&path);
        assert_eq!(project, loaded);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r##"{"version":"1.0","width":800,"height":600,"fps":12,
            "annotations":[],"future_field":42,"nested":{"a":[1,2]}}"##;
        let p = Project::from_json(json).expect("unknown fields should be ignored");
        assert!(p.annotations.is_empty());
        assert_eq!(p.trim_start_ms, 0);
        assert_eq!(p.trim_end_ms, None);
    }

    #[test]
    fn unknown_annotation_type_is_rejected() {
        let json = r##"{"version":"1.0","width":800,"height":600,"fps":12,
            "annotations":[{"type":"sparkle","x":1,"y":1,"start_ms":0,"end_ms":1}]}"##;
        assert!(matches!(
            Project::from_json(json),
            Err(ProjectError::Json(_))
        ));
    }

    #[test]
    fn invalid_projects_are_rejected() {
        for (name, json) in [
            (
                "width 0",
                r##"{"version":"1.0","width":0,"height":600,"fps":12,"annotations":[]}"##,
            ),
            (
                "bad fps",
                r##"{"version":"1.0","width":800,"height":600,"fps":7,"annotations":[]}"##,
            ),
            (
                "bad version",
                r##"{"version":"2.0","width":800,"height":600,"fps":12,"annotations":[]}"##,
            ),
            (
                "missing version",
                r##"{"width":800,"height":600,"fps":12,"annotations":[]}"##,
            ),
            (
                "non string version",
                r##"{"version":1.0,"width":800,"height":600,"fps":12,"annotations":[]}"##,
            ),
            (
                "inverted trim",
                r##"{"version":"1.0","width":800,"height":600,"fps":12,"trim_start_ms":4000,"trim_end_ms":1000,"annotations":[]}"##,
            ),
        ] {
            assert!(
                Project::from_json(json).is_err(),
                "{name} should be rejected"
            );
        }
    }

    #[test]
    fn invalid_annotations_are_rejected_with_clear_error() {
        let inverted = r##"{"version":"1.0","width":800,"height":600,"fps":12,
            "annotations":[{"type":"arrow","x":10,"y":10,"width":50,"height":50,
            "color":"#FF0000","thickness":3,"start_ms":2000,"end_ms":1000}]}"##;
        let err = Project::from_json(inverted).unwrap_err().to_string();
        assert!(err.contains("annotation 0"), "error was: {err}");
        assert!(err.contains("end_ms 1000"), "error was: {err}");

        let out_of_bounds = r##"{"version":"1.0","width":800,"height":600,"fps":12,
            "annotations":[{"type":"blur","x":900,"y":10,"width":50,"height":50,
            "radius":12,"start_ms":0,"end_ms":1000}]}"##;
        let err = Project::from_json(out_of_bounds).unwrap_err().to_string();
        assert!(err.contains("outside recording bounds"), "error was: {err}");
    }
}
