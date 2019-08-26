#![allow(missing_docs)] // FIXME: Document this

use crate::book::BookItem;
use crate::errors::*;
use crate::renderer::{RenderContext, Renderer};
use crate::utils;

use std::fs;

#[derive(Default)]
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        MarkdownRenderer
    }
}

impl Renderer for MarkdownRenderer {
    fn name(&self) -> &str {
        "markdown"
    }

    fn render(&self, ctx: &RenderContext) -> Result<()> {
        let destination = &ctx.destination;
        let book = &ctx.book;

        if destination.exists() {
            utils::fs::remove_dir_content(destination)
                .chain_err(|| "Unable to remove stale Markdown output")?;
        }

        trace!("markdown render");
        for item in book.iter() {
            if let BookItem::Chapter(ref ch) = *item {
                utils::fs::write_file(&ctx.destination, &ch.path, ch.content.as_bytes())?;
            }
        }

        fs::create_dir_all(&destination)
            .chain_err(|| "Unexpected error when constructing destination path")?;

        Ok(())
    }
}
