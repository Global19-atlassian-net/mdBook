use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use toml;

use super::MDBook;
use config::Config;
use snafu::{ResultExt, Snafu};
use theme;

// Used to batch IO operations for the purpose of grouping error
// reporting.
fn try_create_and_write_file<F>(path: &Path, f: F) -> io::Result<()>
where
    F: FnOnce(&mut File) -> io::Result<()>
{
    let mut file = File::create(path)?;
    f(&mut file)
}

#[allow(missing_docs)] // TODO[SNAFU]
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Unable to serialize the config: {}", source))]
    SerializeConfig {
        source: toml::ser::Error,
    },

    #[snafu(display("Unable to write the config to {}: {}", path.display(), source))]
    CreateConfig {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Couldn't create theme directory {}: {}", path.display(), source))]
    CreateThemeDir {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Couldn't copy theme file {}: {}", path.display(), source))]
    CreateThemeFile {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Unable to create .gitignore at {}: {}", path.display(), source))]
    CreateGitignore {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Unable to create summary at {}: {}", path.display(), source))]
    CreateSummary {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Unable to create chapter one at {}: {}", path.display(), source))]
    CreateChapterOne {
        source: io::Error,
        path: PathBuf,
    },

    #[snafu(display("Unable to create scaffolding directory at {}: {}", path.display(), source))]
    CreateScaffoldDirectory {
        source: io::Error,
        path: PathBuf,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// A helper for setting up a new book and its directory structure.
#[derive(Debug, Clone, PartialEq)]
pub struct BookBuilder {
    root: PathBuf,
    create_gitignore: bool,
    config: Config,
    copy_theme: bool,
}

impl BookBuilder {
    /// Create a new `BookBuilder` which will generate a book in the provided
    /// root directory.
    pub fn new<P: Into<PathBuf>>(root: P) -> BookBuilder {
        BookBuilder {
            root: root.into(),
            create_gitignore: false,
            config: Config::default(),
            copy_theme: false,
        }
    }

    /// Set the `Config` to be used.
    pub fn with_config(&mut self, cfg: Config) -> &mut BookBuilder {
        self.config = cfg;
        self
    }

    /// Get the config used by the `BookBuilder`.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Should the theme be copied into the generated book (so users can tweak
    /// it)?
    pub fn copy_theme(&mut self, copy: bool) -> &mut BookBuilder {
        self.copy_theme = copy;
        self
    }

    /// Should we create a `.gitignore` file?
    pub fn create_gitignore(&mut self, create: bool) -> &mut BookBuilder {
        self.create_gitignore = create;
        self
    }

    /// Generate the actual book. This will:
    ///
    /// - Create the directory structure.
    /// - Stub out some dummy chapters and the `SUMMARY.md`.
    /// - Create a `.gitignore` (if applicable)
    /// - Create a themes directory and populate it (if applicable)
    /// - Generate a `book.toml` file,
    /// - Then load the book so we can build it or run tests.
    pub fn build(&self) -> Result<MDBook> {
        info!("Creating a new book with stub content");

        self.create_directory_structure()?;
        self.create_stub_files()?;

        if self.create_gitignore {
            self.build_gitignore()?;
        }

        if self.copy_theme {
            self.copy_across_theme()?
        }

        self.write_book_toml()?;

        match MDBook::load(&self.root) {
            Ok(book) => Ok(book),
            Err(e) => {
                error!("{}", e);

                panic!(
                    "The BookBuilder should always create a valid book. If you are seeing this it \
                     is a bug and should be reported."
                );
            }
        }
    }

    fn write_book_toml(&self) -> Result<()> {
        debug!("Writing book.toml");
        let book_toml = self.root.join("book.toml");
        let cfg = toml::to_vec(&self.config).context(SerializeConfig)?;

        fs::write(&book_toml, cfg)
            .context(CreateConfig { path: &book_toml })?;
        Ok(())
    }

    fn copy_across_theme(&self) -> Result<()> {
        debug!("Copying theme");

        let themedir = self
            .config
            .html_config()
            .and_then(|html| html.theme)
            .unwrap_or_else(|| self.config.book.src.join("theme"));

        let create_theme_dir = |path| {
            fs::create_dir_all(&path).context(CreateThemeDir { path })
        };

        let create_theme_file = |path, contents| {
            fs::write(&path, contents).context(CreateThemeFile { path })
        };

        let themedir = self.root.join(themedir);
        let cssdir = themedir.join("css");

        create_theme_dir(&themedir)?;
        create_theme_dir(&cssdir)?;

        create_theme_file(themedir.join("index.hbs"), theme::INDEX)?;
        create_theme_file(themedir.join("favicon.png"), theme::FAVICON)?;
        create_theme_file(themedir.join("book.js"), theme::JS)?;
        create_theme_file(themedir.join("highlight.css"), theme::HIGHLIGHT_CSS)?;
        create_theme_file(themedir.join("highlight.js"), theme::HIGHLIGHT_JS)?;

        create_theme_file(cssdir.join("general.css"), theme::GENERAL_CSS)?;
        create_theme_file(cssdir.join("chrome.css"), theme::CHROME_CSS)?;
        create_theme_file(cssdir.join("print.css"), theme::PRINT_CSS)?;
        create_theme_file(cssdir.join("variables.css"), theme::VARIABLES_CSS)?;

        Ok(())
    }

    fn build_gitignore(&self) -> Result<()> {
        debug!("Creating .gitignore");

        let gitignore = self.root.join(".gitignore");
        try_create_and_write_file(&gitignore, |f| {
            writeln!(f, "{}", self.config.build.build_dir.display())?;
            Ok(())
        }).context(CreateGitignore { path: gitignore })
    }

    fn create_stub_files(&self) -> Result<()> {
        debug!("Creating example book contents");
        let src_dir = self.root.join(&self.config.book.src);

        let summary = src_dir.join("SUMMARY.md");
        if !summary.exists() {
            trace!("No summary found creating stub summary and chapter_1.md.");
            try_create_and_write_file(&summary, |f| {
                writeln!(f, "# Summary")?;
                writeln!(f)?;
                writeln!(f, "- [Chapter 1](./chapter_1.md)")?;
                Ok(())
            }).context(CreateSummary { path: summary })?;

            let chapter_1 = src_dir.join("chapter_1.md");
            try_create_and_write_file(&chapter_1, |f| {
                writeln!(f, "# Chapter 1")?;
                Ok(())
            }).context(CreateChapterOne { path: chapter_1 })?;
        } else {
            trace!("Existing summary found, no need to create stub files.");
        }
        Ok(())
    }

    fn create_directory_structure(&self) -> Result<()> {
        debug!("Creating directory tree");
        fs::create_dir_all(&self.root).context(CreateScaffoldDirectory { path: &self.root })?;

        let src = self.root.join(&self.config.book.src);
        fs::create_dir_all(&src).context(CreateScaffoldDirectory { path: &src })?;

        let build = self.root.join(&self.config.build.build_dir);
        fs::create_dir_all(&build).context(CreateScaffoldDirectory { path: &build })?;

        Ok(())
    }
}
