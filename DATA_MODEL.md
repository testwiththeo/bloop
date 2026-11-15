# DATA_MODEL.md, Project File Format

## 1. Overview

Bloop saves the editable project as a JSON file with the extension `.bloop.json`. This file contains everything needed to re-open the project, edit annotations, and re-export the GIF with different settings, without re-recording.

The exported GIF itself does not contain the project data. The GIF is a flat, final artifact. The `.bloop.json` is the editable source of truth.

Design goals for the format:

- Human readable: a user should be able to open it in a text editor and understand it
- Forward compatible: old files must load in new versions
- Strict where it matters: invalid coordinates or inverted time ranges must be rejected, not silently fixed
- Minimal: no redundant fields, no "we might need this later" fields

## 2. File Structure

### Top Level Project

```json
{
  "version": "1.0",
  "width": 800,
  "height": 600,
  "fps": 12,
  "trim_start_ms": 0,
  "trim_end_ms": 5000,
  "annotations": []
}
```

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `version` | string | Yes | Format version. Currently "1.0". Used for migration. |
| `width` | integer | Yes | Recording width in pixels. Must be greater than 0. |
| `height` | integer | Yes | Recording height in pixels. Must be greater than 0. |
| `fps` | integer | Yes | Frames per second for export. Allowed: 5, 10, 12, 15, 20. |
| `trim_start_ms` | integer | No, default 0 | Start of the kept region, in milliseconds. Must be 0 or more. |
| `trim_end_ms` | integer | No, default equals duration | End of the kept region, in milliseconds. Must be greater than `trim_start_ms`. |
| `annotations` | array | Yes | List of annotation objects. Can be empty. |

### Common Annotation Fields

Every annotation, regardless of type, has these fields:

| Field | Type | Description |
| --- | --- | --- |
| `type` | string | One of: "arrow", "box", "text", "highlight", "blur" |
| `x` | integer | Left edge in pixels, relative to the recording area. Must be 0 or more and less than `width`. |
| `y` | integer | Top edge in pixels. Must be 0 or more and less than `height`. |
| `start_ms` | integer | Time when the annotation appears, in milliseconds. Must be 0 or more. |
| `end_ms` | integer | Time when the annotation disappears. Must be greater than `start_ms`. |

## 3. Annotation Types in Detail

### Arrow

An arrow points from its tail at `(x, y)` to its tip at `(tip_x, tip_y)`. The arrowhead sits at the tip. The tip can be in any direction from the tail, including up and left.

```json
{
  "type": "arrow",
  "x": 120,
  "y": 80,
  "tip_x": 320,
  "tip_y": 120,
  "color": "#FF0000",
  "thickness": 3,
  "start_ms": 1000,
  "end_ms": 3000
}
```

- `x`, `y`: tail point in pixels.
- `tip_x`, `tip_y`: tip point in pixels, where the arrowhead is drawn.
- Compatibility: files written before this change stored `width` and `height` instead of `tip_x` and `tip_y`, with the tip always at the bottom-right of the bounding box. The loader converts them: `tip_x = x + width`, `tip_y = y + height`.

- `color`: HEX string, for example "#FF0000". Must match `^#[0-9A-Fa-f]{6}$`.
- `thickness`: integer, 1 to 10, default 3. Line thickness in pixels.

### Box

A rectangle outline around a region. Used for layout bugs, misaligned elements, or to draw attention to a panel.

```json
{
  "type": "box",
  "x": 100,
  "y": 100,
  "width": 200,
  "height": 100,
  "color": "#FF0000",
  "thickness": 3,
  "start_ms": 1000,
  "end_ms": 3000
}
```

### Text

A short label. Text is rendered with a background pill so it remains readable over any content. The text is limited to 140 characters to keep GIFs readable. If you need more than 140 characters, you should write it in the bug report, not on the GIF.

```json
{
  "type": "text",
  "x": 100,
  "y": 100,
  "text": "Clicking here does nothing",
  "color": "#FFFFFF",
  "background": "#FF0000",
  "font_size": 14,
  "start_ms": 1000,
  "end_ms": 3000
}
```

- `text`: string, 1 to 140 characters. Must not be empty.
- `background`: HEX color for the pill background.

### Highlight

A translucent fill over a region. Unlike a box, which is just an outline, highlight fills the area with a semi transparent color, like a highlighter pen. Useful for "look at this row in the table."

```json
{
  "type": "highlight",
  "x": 100,
  "y": 100,
  "width": 200,
  "height": 100,
  "color": "#FFE500",
  "opacity": 0.4,
  "start_ms": 1000,
  "end_ms": 3000
}
```

- `opacity`: float, 0.0 to 1.0, default 0.4.

### Blur

A blur or mosaic over a region. This is the only annotation that is privacy critical. It must actually obscure data in the exported GIF. The blur is applied at export time by blurring the underlying pixels, not by drawing a semi transparent rectangle that could be removed.

```json
{
  "type": "blur",
  "x": 100,
  "y": 100,
  "width": 200,
  "height": 100,
  "radius": 12,
  "start_ms": 1000,
  "end_ms": 3000
}
```

- `radius`: integer, blur radius in pixels. Higher means more obscured. Default 12.

## 4. Validation Rules

When loading a `.bloop.json` file, Bloop must:

1. Reject the file if `version` is missing or is not a string.
2. Reject the file if `width` or `height` is missing, zero, or negative.
3. Reject the file if `fps` is not one of the allowed values.
4. Reject the file if any annotation has `start_ms` greater than or equal to `end_ms`.
5. Reject the file if any annotation has `x` or `y` outside the recording bounds.
6. Ignore unknown fields, forward compatibility. Do not error on them.
7. Reject unknown annotation types, because silently dropping an annotation would be data loss and the user would not know why their arrow disappeared.

If a file fails validation, show a clear error: "This project file is invalid: annotation 2 has end_ms 1000 before start_ms 2000." Do not show a generic "parse error."

## 5. Versioning and Migration

- `version` is a string, not a float. "1.0" is correct, 1.0 is not.
- When a new version adds a field, the loader must provide a default for old files. For example, if v1.1 adds `thickness` to arrows, old files without `thickness` must load with `thickness` equal to 3.
- When a new version removes a field, the loader must ignore it in old files.
- There must be a test for every migration: load a v1.0 file in the current code and assert that it still works.

## 6. Example: Full Project

```json
{
  "version": "1.0",
  "width": 800,
  "height": 600,
  "fps": 12,
  "trim_start_ms": 500,
  "trim_end_ms": 4500,
  "annotations": [
    {
      "type": "arrow",
      "x": 120,
      "y": 80,
      "tip_x": 320,
      "tip_y": 120,
      "color": "#FF0000",
      "thickness": 3,
      "start_ms": 1000,
      "end_ms": 3000
    },
    {
      "type": "text",
      "x": 100,
      "y": 200,
      "text": "Clicking here does nothing",
      "color": "#FFFFFF",
      "background": "#FF0000",
      "font_size": 14,
      "start_ms": 1000,
      "end_ms": 3000
    },
    {
      "type": "blur",
      "x": 500,
      "y": 10,
      "width": 200,
      "height": 30,
      "radius": 12,
      "start_ms": 0,
      "end_ms": 4500
    }
  ]
}
```

This is a realistic project: a 4 second trimmed clip, an arrow and a label for 2 seconds, and a blur over an email address for the whole clip.
