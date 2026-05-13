CREATE INDEX IF NOT EXISTS idx_novel_tag_tag_id_novel_id
ON novel_tag (tag_id, novel_id);

CREATE INDEX IF NOT EXISTS idx_novel_is_limit
ON novel (is_limit);

CREATE INDEX IF NOT EXISTS idx_novel_reply_count
ON novel (reply_count);
