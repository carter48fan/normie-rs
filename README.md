# normie-rs 🧹

A blazing-fast, terminal-based tabular data normalization tool. `normie-rs` helps you quickly clean up messy CSV, TSV, and tabular data by mapping unique column values to normalized targets, with a focus on speed and keyboard-driven efficiency.

![normie-rs Splash](./assets/splash.avif)

## Features

- **Universal Tabulating Format Support**: Auto-detects delimiters (CSV `,`, TSV `\t`, Semicolon `;`, Pipe `|`, Colon `:`) and preserves formatting when saving.
- **Universal Encoding & Mojibake Healing**: Seamlessly loads UTF-8 (with/without BOM), UTF-16 LE/BE (Excel exports), Windows-1252, and repairs C1 control characters (U+0080..=U+009F) and double-encoded UTF-8.
- **Fault-Tolerant & Resilient**: Gracefully parses unescaped quotes (e.g. `8" Banjo Rim`), unclosed quotes, and ragged rows without crashing.
- **CLI Direct Opening**: Launch with `normie-rs path/to/file.tsv` to immediately open in the normalizer.
- **Dashboard & Splash Screen**: Quick access to recent files or start a new session.
- **TUI File Browser**: Navigate your local file system, filter tabular formats (`.csv`, `.tsv`, `.tab`, `.txt`, `.psv`, `.dsv`), or toggle to view all files.
- **Recent File Tracking**: Remembers your last 10 opened files for instant access.
- **Intelligent Normalization**: 
  - View unique values per column.
  - Search and filter values.
  - Batch map selected values to a single target.
  - Side-by-side view of source keys (IDs) to help you identify values in context.
- **Persistent Mappings**: Saves your mapping progress to a `.normie.json` file next to your data, so you can resume later.
- **Blazing Fast**: Built in Rust with `ratatui` for a snappy, low-latency experience.

## Installation

### Binary Releases

You can download pre-compiled binaries for your platform from the [v1.1.1 Release](https://github.com/SirMarshall/normie-rs/releases/tag/v1.1.1).

### From Source

Ensure you have [Rust](https://rustup.rs/) and `cargo` installed.

```bash
git clone https://github.com/SirMarshall/normie-rs.git
cd normie-rs
cargo install --path .
```

## Usage

Simply run the executable, or provide a file directly:

```bash
normie-rs
# Or open a file directly:
normie-rs path/to/products.tsv
```

### Global Shortcuts
- `CTRL+C`: Force quit the application at any time.

### Keyboard Shortcuts

#### Dashboard
- `UP/DOWN`: Navigate menu.
- `ENTER`: Select option.
- `Q / ESC`: Quit.

#### File Browser
- `UP/DOWN`: Navigate files.
- `ENTER`: Open file or enter Directory.
- `BACKSPACE`: Go to parent directory.
- `A`: Toggle between tabular data files and all files.
- `Q / ESC`: Return to Dashboard.

#### Normalizer
- `TAB`: Switch between **Columns** and **Unique Values** panes.
- `M`: Toggle **Master Transformation Pipeline** view.
- `UP/DOWN`: Navigate the active list.
- `SHIFT+UP/DOWN` or `J/K`: Reorder transformation step (in Master view).
- `SPACE`: Multi-select values for batch mapping or filtering.
- `ENTER`: Open **Normalization Modal** to map selected/highlighted value(s).
- `/`: Open **Search Modal** to filter unique values in the current column.
- `F`: Open **Conditional Filter Modal** (or clear filter if already set).
- `K`: Open **Key Selection Modal** to change the Source Key column.
- `S`: Save normalized CSV.
- `DELETE / BACKSPACE`: Remove a mapping (or a pipeline step in Master view).
- `Q / ESC`: Return to Dashboard.

#### Input Modals (Normalize / Filter)
- `LEFT / RIGHT`: Move cursor.
- `HOME / END`: Move cursor to beginning / end.
- `CTRL+U`: Clear text buffer.
- `CTRL+W`: Delete previous word.
- `ENTER`: Confirm.
- `ESC`: Cancel.

## Data & Configuration

### CSV Requirements
`normie-rs` requires a header row to correctly identify and map columns.

### Configuration
Your recent files and application state are stored locally:
- **Linux**: `~/.config/normie-rs/config.json`
- **macOS**: `~/Library/Application Support/com.GoldTone.normie-rs/config.json`
- **Windows**: `%AppData%\GoldTone\normie-rs\config\config.json`

## Output

When you save your progress, `normie-rs` creates:
1. `{filename}_normalized.csv`: The final cleaned data.
2. `{filename}.normie.json`: A mapping file that allows you to reload your progress in future sessions.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License.
