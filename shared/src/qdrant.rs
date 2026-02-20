use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, Distance, Filter, PointStruct,
    ScrollPointsBuilder, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const EMBEDDING_DIM: u64 = 768;

/// KnowledgeBase - Qdrant-powered semantic code search
/// Matches existing collection schema: filePath, codeChunk, startLine, endLine, segmentHash, pathSegments
/// 768-dim cosine distance (matches Qwen3-Embedding-0.6B output)
pub struct KnowledgeBase {
    client: Qdrant,
    http: Client,
    embedding_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub code_chunk: String,
    pub start_line: u64,
    pub end_line: u64,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub points_count: u64,
}

impl KnowledgeBase {
    /// Connect to Qdrant and configure embedding endpoint
    pub async fn connect(qdrant_url: &str, embedding_port: u16) -> Result<Self, String> {
        // qdrant-client uses gRPC (port 6334), not REST (6333)
        // Auto-fix if user provides the REST URL
        let grpc_url = qdrant_url.replace(":6333", ":6334");
        let mut builder = Qdrant::from_url(&grpc_url);
        builder.check_compatibility = false;
        let client = builder
            .build()
            .map_err(|e| format!("Qdrant connection failed: {e}"))?;

        // Verify connection
        client
            .list_collections()
            .await
            .map_err(|e| format!("Qdrant not reachable: {e}"))?;

        Ok(Self {
            client,
            http: Client::new(),
            embedding_url: format!("http://localhost:{embedding_port}/v1/embeddings"),
        })
    }

    /// Get embedding from local model (OpenAI-compatible endpoint)
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let body = serde_json::json!({
            "input": text,
            "model": "text-embedding"
        });

        let resp = self
            .http
            .post(&self.embedding_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Embedding request failed: {e}"))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Embedding parse failed: {e}"))?;

        let embedding = json["data"][0]["embedding"]
            .as_array()
            .ok_or("No embedding in response")?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect::<Vec<f32>>();

        if embedding.len() != EMBEDDING_DIM as usize {
            return Err(format!(
                "Expected {EMBEDDING_DIM} dims, got {}",
                embedding.len()
            ));
        }

        Ok(embedding)
    }

    /// Semantic search across indexed code
    /// Falls back to keyword search if no embedding model is available
    pub async fn search(
        &self,
        query: &str,
        collection: &str,
        limit: u64,
        path_filter: Option<&str>,
    ) -> Result<Vec<SearchResult>, String> {
        // Try vector search first
        match self.embed(query).await {
            Ok(query_vec) => {
                let mut search_builder = SearchPointsBuilder::new(collection, query_vec, limit)
                    .with_payload(true);

                if let Some(path) = path_filter {
                    let filter = Filter::must([Condition::matches("filePath", path.to_string())]);
                    search_builder = search_builder.filter(filter);
                }

                let results = self
                    .client
                    .search_points(search_builder)
                    .await
                    .map_err(|e| format!("Search failed: {e}"))?;

                Ok(results
                    .result
                    .iter()
                    .map(|point| self.point_to_result(&point.payload, point.score))
                    .collect())
            }
            Err(_) => {
                // Fallback: keyword search via scroll + client-side filtering
                self.keyword_search(query, collection, limit, path_filter).await
            }
        }
    }

    /// Keyword search fallback - scrolls collection and filters by text match
    async fn keyword_search(
        &self,
        query: &str,
        collection: &str,
        limit: u64,
        path_filter: Option<&str>,
    ) -> Result<Vec<SearchResult>, String> {
        let keywords: Vec<String> = query.to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if keywords.is_empty() {
            return Ok(vec![]);
        }

        // Scroll in batches and filter client-side
        let batch_size = 100u32;
        let mut results: Vec<SearchResult> = Vec::new();
        let mut offset: Option<qdrant_client::qdrant::PointId> = None;

        // Scan up to 1000 points max to keep it fast
        for _ in 0..10 {
            let mut scroll = ScrollPointsBuilder::new(collection)
                .with_payload(true)
                .limit(batch_size);

            if let Some(ref off) = offset {
                scroll = scroll.offset(off.clone());
            }

            let batch = self
                .client
                .scroll(scroll)
                .await
                .map_err(|e| format!("Keyword search failed: {e}"))?;

            for point in &batch.result {
                let payload = &point.payload;
                let code = payload
                    .get("codeChunk")
                    .and_then(|v| v.as_str())
                    .cloned()
                    .unwrap_or_default();
                let file_path = payload
                    .get("filePath")
                    .and_then(|v| v.as_str())
                    .cloned()
                    .unwrap_or_default();

                // Apply path filter if provided
                if let Some(pf) = path_filter {
                    if !file_path.contains(pf) {
                        continue;
                    }
                }

                let lower_code = code.to_lowercase();
                let lower_path = file_path.to_lowercase();

                // Score by how many keywords match
                let matches: usize = keywords.iter()
                    .filter(|kw| lower_code.contains(kw.as_str()) || lower_path.contains(kw.as_str()))
                    .count();

                if matches > 0 {
                    let score = matches as f32 / keywords.len() as f32;
                    results.push(SearchResult {
                        file_path,
                        code_chunk: code,
                        start_line: payload.get("startLine").and_then(|v| v.as_integer()).unwrap_or(0) as u64,
                        end_line: payload.get("endLine").and_then(|v| v.as_integer()).unwrap_or(0) as u64,
                        score,
                    });
                }
            }

            // Check if we have enough or no more data
            if results.len() >= limit as usize || batch.next_page_offset.is_none() {
                break;
            }
            offset = batch.next_page_offset;
        }

        // Sort by score descending, take top N
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit as usize);

        Ok(results)
    }

    fn point_to_result(&self, payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>, score: f32) -> SearchResult {
        SearchResult {
            file_path: payload
                .get("filePath")
                .and_then(|v| v.as_str())
                .cloned()
                .unwrap_or_default(),
            code_chunk: payload
                .get("codeChunk")
                .and_then(|v| v.as_str())
                .cloned()
                .unwrap_or_default(),
            start_line: payload
                .get("startLine")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u64,
            end_line: payload
                .get("endLine")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u64,
            score,
        }
    }

    /// Index a project: walk files, chunk by logical boundaries, embed, upsert
    pub async fn index_project(
        &self,
        project_path: &Path,
        collection: &str,
    ) -> Result<usize, String> {
        self.index_directory(project_path, project_path, collection).await
    }

    /// Index a specific subdirectory into a named collection
    /// Indexes ALL files: text files get chunked, binary files (images, PDFs) get metadata points
    /// Falls back to zero vectors if no embedding model is available (keyword search still works)
    pub async fn index_directory(
        &self,
        project_root: &Path,
        target_dir: &Path,
        collection: &str,
    ) -> Result<usize, String> {
        self.ensure_collection(collection).await?;

        // Check if embedding model is available
        let has_embeddings = self.embed("test").await.is_ok();

        let walker = ignore::WalkBuilder::new(target_dir)
            .max_depth(Some(8))
            .build();

        let mut total_points = 0;
        let mut batch: Vec<PointStruct> = Vec::new();

        // Binary extensions to skip entirely (compiled, archives, caches)
        let skip_extensions = [
            "exe", "dll", "so", "dylib", "wasm", "o", "a", "lib",
            "zip", "tar", "gz", "bz2", "7z", "rar", "xz",
            "class", "pyc", "pyo", "cache", "DS_Store",
            "lock", "map", "min.js", "min.css",
            "woff", "woff2", "ttf", "eot", "otf",
            "ico", "mp3", "mp4", "wav", "avi", "mov",
        ];

        // Image extensions - index as metadata
        let image_extensions = ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp", "tiff", "ico"];

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let rel_path = path
                .strip_prefix(project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Skip unindexable binary files
            if skip_extensions.contains(&ext.as_str()) {
                continue;
            }

            let chunks: Vec<CodeChunk>;

            if ext == "pdf" {
                // PDF: extract text using pdf-extract
                chunks = match extract_pdf_text(path) {
                    Some(text) if !text.trim().is_empty() => chunk_code(&text, &rel_path),
                    _ => {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        vec![CodeChunk {
                            text: format!("[PDF Document] File: {} Path: {}", filename, rel_path),
                            start_line: 0,
                            end_line: 0,
                        }]
                    }
                };
            } else if ext == "docx" {
                // Word documents: extract text from ZIP/XML
                chunks = match extract_docx_text(path) {
                    Some(text) if !text.trim().is_empty() => chunk_code(&text, &rel_path),
                    _ => {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        vec![CodeChunk {
                            text: format!("[Word Document] File: {} Path: {}", filename, rel_path),
                            start_line: 0,
                            end_line: 0,
                        }]
                    }
                };
            } else if ext == "xlsx" {
                // Excel spreadsheets: extract text from ZIP/XML
                chunks = match extract_xlsx_text(path) {
                    Some(text) if !text.trim().is_empty() => chunk_code(&text, &rel_path),
                    _ => {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        vec![CodeChunk {
                            text: format!("[Excel Spreadsheet] File: {} Path: {}", filename, rel_path),
                            start_line: 0,
                            end_line: 0,
                        }]
                    }
                };
            } else if image_extensions.contains(&ext.as_str()) {
                // Images: metadata point with filename as searchable text
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                chunks = vec![CodeChunk {
                    text: format!("[Image: {}] Path: {} Type: {}", filename, rel_path, ext),
                    start_line: 0,
                    end_line: 0,
                }];
            } else {
                // Try to read as text
                let content = match std::fs::read_to_string(path) {
                    Ok(c) if !c.is_empty() => c,
                    _ => continue,
                };

                let mut file_chunks = chunk_code(&content, &rel_path);

                // For markdown files, extract and index links separately
                if ext == "md" {
                    if let Some(links_chunk) = extract_markdown_links(&content, &rel_path) {
                        file_chunks.push(links_chunk);
                    }
                }

                chunks = file_chunks;
            }

            for chunk in &chunks {
                let embedding = if has_embeddings {
                    self.embed(&chunk.text).await.unwrap_or_else(|_| vec![0.0; EMBEDDING_DIM as usize])
                } else {
                    vec![0.0; EMBEDDING_DIM as usize]
                };

                let hash = segment_hash(&chunk.text);
                let id = hash_to_u64(&format!("{}:{}:{}", rel_path, chunk.start_line, hash));

                let payload: Payload = serde_json::json!({
                    "filePath": rel_path,
                    "codeChunk": chunk.text,
                    "startLine": chunk.start_line,
                    "endLine": chunk.end_line,
                    "segmentHash": hash,
                    "pathSegments": rel_path.split('/').collect::<Vec<_>>(),
                })
                .try_into()
                .unwrap();

                batch.push(PointStruct::new(id, embedding, payload));

                if batch.len() >= 20 {
                    let count = batch.len();
                    self.client
                        .upsert_points(UpsertPointsBuilder::new(collection, std::mem::take(&mut batch)))
                        .await
                        .map_err(|e| format!("Upsert failed: {e}"))?;
                    total_points += count;
                }
            }
        }

        if !batch.is_empty() {
            let count = batch.len();
            self.client
                .upsert_points(UpsertPointsBuilder::new(collection, batch))
                .await
                .map_err(|e| format!("Upsert failed: {e}"))?;
            total_points += count;
        }

        Ok(total_points)
    }

    /// List all collections with point counts
    pub async fn list_collections(&self) -> Result<Vec<CollectionInfo>, String> {
        let collections = self
            .client
            .list_collections()
            .await
            .map_err(|e| format!("List collections failed: {e}"))?;

        let mut result = Vec::new();
        for c in &collections.collections {
            let info = self
                .client
                .collection_info(&c.name)
                .await
                .ok();

            let points_count = info
                .and_then(|i| i.result)
                .map(|r| r.points_count.unwrap_or(0))
                .unwrap_or(0);

            result.push(CollectionInfo {
                name: c.name.clone(),
                points_count,
            });
        }

        Ok(result)
    }

    /// Browse collection without embeddings - just scroll through stored data
    pub async fn browse(
        &self,
        collection: &str,
        limit: u64,
    ) -> Result<Vec<SearchResult>, String> {
        let scroll = ScrollPointsBuilder::new(collection)
            .with_payload(true)
            .limit(limit as u32);

        let results = self
            .client
            .scroll(scroll)
            .await
            .map_err(|e| format!("Browse failed: {e}"))?;

        let browse_results = results
            .result
            .iter()
            .map(|point| {
                let payload = &point.payload;
                SearchResult {
                    file_path: payload
                        .get("filePath")
                        .and_then(|v| v.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    code_chunk: payload
                        .get("codeChunk")
                        .and_then(|v| v.as_str())
                        .cloned()
                        .unwrap_or_default(),
                    start_line: payload
                        .get("startLine")
                        .and_then(|v| v.as_integer())
                        .unwrap_or(0) as u64,
                    end_line: payload
                        .get("endLine")
                        .and_then(|v| v.as_integer())
                        .unwrap_or(0) as u64,
                    score: 0.0,
                }
            })
            .collect();

        Ok(browse_results)
    }

    /// Ensure a collection exists with the right schema
    pub async fn ensure_project_collection(&self, name: &str) -> Result<(), String> {
        self.ensure_collection(name).await
    }

    async fn ensure_collection(&self, name: &str) -> Result<(), String> {
        let exists = self
            .client
            .collection_exists(name)
            .await
            .map_err(|e| format!("Collection check failed: {e}"))?;

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(name)
                        .vectors_config(VectorParamsBuilder::new(EMBEDDING_DIM, Distance::Cosine)),
                )
                .await
                .map_err(|e| format!("Create collection failed: {e}"))?;
        }

        Ok(())
    }
}

// ──────────────────────────────────────
// Code chunking utilities
// ──────────────────────────────────────

struct CodeChunk {
    text: String,
    start_line: u64,
    end_line: u64,
}

/// Chunk code by logical boundaries (functions, impl blocks, etc.)
/// Falls back to fixed-size chunks if no boundaries found
fn chunk_code(content: &str, _path: &str) -> Vec<CodeChunk> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return vec![];
    }

    // For small files, use the whole thing
    if lines.len() <= 50 {
        return vec![CodeChunk {
            text: content.to_string(),
            start_line: 1,
            end_line: lines.len() as u64,
        }];
    }

    // Chunk by ~40 line windows with 10 line overlap
    let chunk_size = 40;
    let overlap = 10;
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let end = (start + chunk_size).min(lines.len());
        let chunk_text = lines[start..end].join("\n");

        chunks.push(CodeChunk {
            text: chunk_text,
            start_line: (start + 1) as u64,
            end_line: end as u64,
        });

        if end >= lines.len() {
            break;
        }
        start += chunk_size - overlap;
    }

    chunks
}

/// Extract text content from a PDF file
fn extract_pdf_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    pdf_extract::extract_text_from_mem(&bytes).ok()
}

/// Extract text from a .docx file (ZIP containing word/document.xml)
fn extract_docx_text(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut xml_content = String::new();
    {
        let mut doc = archive.by_name("word/document.xml").ok()?;
        std::io::Read::read_to_string(&mut doc, &mut xml_content).ok()?;
    }

    // Extract text from <w:t> elements and add paragraph breaks at </w:p>
    Some(strip_docx_xml(&xml_content))
}

/// Strip XML tags from docx document.xml, preserving paragraph structure
fn strip_docx_xml(xml: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut current_tag = String::new();

    for ch in xml.chars() {
        match ch {
            '<' => {
                in_tag = true;
                current_tag.clear();
            }
            '>' => {
                in_tag = false;
                // Add newline after paragraph ends
                if current_tag.starts_with("/w:p") || current_tag.starts_with("w:p/") {
                    result.push('\n');
                }
                // Add tab for table cells
                if current_tag.starts_with("/w:tc") {
                    result.push('\t');
                }
                current_tag.clear();
            }
            _ if in_tag => {
                current_tag.push(ch);
            }
            _ => {
                result.push(ch);
            }
        }
    }

    // Clean up excessive whitespace while preserving line breaks
    let lines: Vec<&str> = result.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

/// Extract text from a .xlsx file (ZIP containing shared strings + sheets)
fn extract_xlsx_text(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut text_parts: Vec<String> = Vec::new();

    // Read shared strings table (most text lives here)
    if let Ok(mut shared) = archive.by_name("xl/sharedStrings.xml") {
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut shared, &mut xml).ok();
        // Extract text from <t> elements
        let re = regex::Regex::new(r"<t[^>]*>([^<]+)</t>").ok()?;
        for cap in re.captures_iter(&xml) {
            let text = cap[1].trim();
            if !text.is_empty() {
                text_parts.push(text.to_string());
            }
        }
    }

    // Also try reading sheet data for inline strings
    for i in 1..=5 {
        let sheet_name = format!("xl/worksheets/sheet{i}.xml");
        if let Ok(mut sheet) = archive.by_name(&sheet_name) {
            let mut xml = String::new();
            std::io::Read::read_to_string(&mut sheet, &mut xml).ok();
            // Extract inline values
            let re = regex::Regex::new(r"<v>([^<]+)</v>").ok()?;
            for cap in re.captures_iter(&xml) {
                let val = cap[1].trim();
                // Only include text-like values (skip pure numbers)
                if !val.is_empty() && val.parse::<f64>().is_err() {
                    text_parts.push(val.to_string());
                }
            }
        }
    }

    if text_parts.is_empty() {
        return None;
    }

    Some(text_parts.join("\n"))
}

/// Extract markdown links and URLs from content, return as a searchable chunk
fn extract_markdown_links(content: &str, rel_path: &str) -> Option<CodeChunk> {
    let link_re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").ok()?;
    let url_re = regex::Regex::new(r"https?://[^\s\)>\]]+").ok()?;

    let mut links: Vec<String> = Vec::new();

    // Extract markdown links: [text](url)
    for cap in link_re.captures_iter(content) {
        let text = &cap[1];
        let url = &cap[2];
        links.push(format!("{} → {}", text, url));
    }

    // Extract bare URLs not already captured
    for mat in url_re.find_iter(content) {
        let url = mat.as_str();
        if !links.iter().any(|l| l.contains(url)) {
            links.push(url.to_string());
        }
    }

    if links.is_empty() {
        return None;
    }

    Some(CodeChunk {
        text: format!("[Links from {}]\n{}", rel_path, links.join("\n")),
        start_line: 0,
        end_line: 0,
    })
}

fn segment_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn hash_to_u64(input: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    u64::from_le_bytes(result[..8].try_into().unwrap())
}
