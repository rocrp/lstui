use anyhow::{Context, Result, anyhow};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StoryRaw {
    pub short_id: String,
    pub created_at: String,
    pub title: String,
    pub url: Option<String>,
    pub score: i64,
    pub comment_count: i64,
    pub submitter_user: String,
    pub tags: Vec<String>,
    pub short_id_url: String,
    pub comments_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StoryWithCommentsRaw {
    #[serde(flatten)]
    pub story: StoryRaw,
    pub comments: Vec<CommentRaw>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CommentRaw {
    pub short_id: String,
    pub created_at: String,
    pub is_deleted: bool,
    pub is_moderated: bool,
    pub parent_comment: Option<String>,
    pub comment_plain: String,
    pub depth: usize,
    pub commenting_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub short_id: String,
    pub title: String,
    pub url: Option<String>,
    pub score: i64,
    pub by: String,
    pub time: i64,
    pub comment_count: i64,
    pub tags: Vec<String>,
    pub short_id_url: String,
    pub comments_url: String,
}

impl Story {
    pub(crate) fn from_raw(raw: StoryRaw) -> Result<Self> {
        let time = parse_created_at(&raw.created_at)?;
        Ok(Self {
            short_id: raw.short_id,
            title: raw.title,
            url: raw.url,
            score: raw.score,
            by: raw.submitter_user,
            time,
            comment_count: raw.comment_count,
            tags: raw.tags,
            short_id_url: raw.short_id_url,
            comments_url: raw.comments_url,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: String,
    pub by: Option<String>,
    pub time: Option<i64>,
    pub text: String,
    pub kids: Vec<String>,
    pub depth: usize,
    pub collapsed: bool,
    pub children_loading: bool,
    pub deleted: bool,
    pub moderated: bool,
}

#[derive(Debug, Clone)]
pub struct CommentNode {
    pub comment: Comment,
    pub children: Vec<CommentNode>,
}

pub(crate) fn parse_created_at(created_at: &str) -> Result<i64> {
    let dt = DateTime::parse_from_rfc3339(created_at)
        .with_context(|| format!("parse created_at rfc3339: {created_at}"))?;
    Ok(dt.timestamp())
}

pub(crate) fn build_comment_tree(comments: Vec<CommentRaw>) -> Result<Vec<CommentNode>> {
    if comments.is_empty() {
        return Ok(Vec::new());
    }

    let mut comment_by_id: HashMap<String, CommentRaw> = HashMap::with_capacity(comments.len());
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut ordered_ids: Vec<String> = Vec::with_capacity(comments.len());
    let mut root_ids = Vec::new();

    for comment in comments {
        let id = comment.short_id.clone();
        ordered_ids.push(id.clone());
        if comment_by_id.insert(id.clone(), comment).is_some() {
            return Err(anyhow!("duplicate comment id={id}"));
        }
    }

    for id in &ordered_ids {
        let comment = comment_by_id.get(id).expect("id present in comment_by_id");
        match comment.parent_comment.as_ref() {
            Some(parent_id) => {
                if !comment_by_id.contains_key(parent_id) {
                    return Err(anyhow!(
                        "comment parent missing id={id} parent_id={parent_id}"
                    ));
                }
                children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(id.clone());
            }
            None => root_ids.push(id.clone()),
        }
    }

    if root_ids.is_empty() {
        return Err(anyhow!("comments present but no root comments"));
    }

    fn build_node(
        id: &str,
        expected_depth: usize,
        comment_by_id: &HashMap<String, CommentRaw>,
        children: &HashMap<String, Vec<String>>,
    ) -> Result<CommentNode> {
        let raw = comment_by_id
            .get(id)
            .ok_or_else(|| anyhow!("comment not found id={id}"))?;
        if raw.depth != expected_depth {
            return Err(anyhow!(
                "comment depth mismatch id={id}: api={} expected={expected_depth}",
                raw.depth
            ));
        }

        let kids = children.get(id).cloned().unwrap_or_default();

        let time = Some(parse_created_at(&raw.created_at)?);
        let deleted = raw.is_deleted;
        let moderated = raw.is_moderated;
        let by = raw.commenting_user.clone().filter(|s| !s.trim().is_empty());
        let mut text = raw.comment_plain.replace('\r', "");
        if text.trim().is_empty() {
            text = if deleted {
                "[deleted]".to_string()
            } else if moderated {
                "[moderated]".to_string()
            } else {
                "[no text]".to_string()
            };
        }

        let comment = Comment {
            id: id.to_string(),
            by,
            time,
            text,
            kids: kids.clone(),
            depth: expected_depth,
            collapsed: !kids.is_empty(),
            children_loading: false,
            deleted,
            moderated,
        };

        let mut child_nodes = Vec::with_capacity(kids.len());
        for child_id in &kids {
            child_nodes.push(build_node(
                child_id,
                expected_depth + 1,
                comment_by_id,
                children,
            )?);
        }

        Ok(CommentNode {
            comment,
            children: child_nodes,
        })
    }

    let mut roots = Vec::with_capacity(root_ids.len());
    for root_id in &root_ids {
        roots.push(build_node(root_id, 0, &comment_by_id, &children)?);
    }
    Ok(roots)
}
