CREATE TABLE novel_new
(
    id                  integer NOT NULL PRIMARY key,
    name                text    NOT NULL,
    desc                text    NOT NULL,
    is_limit            boolean NOT NULL,
    latest_chapter_name text    NOT NULL,
    latest_chapter_id   integer NOT NULL,
    word_count          integer NOT NULL,
    read_count          integer,
    reply_count         integer,
    author_id           integer,
    author_name         text    not null
);

INSERT INTO novel_new (
    id,
    name,
    desc,
    is_limit,
    latest_chapter_name,
    latest_chapter_id,
    word_count,
    read_count,
    reply_count,
    author_id,
    author_name
)
SELECT
    id,
    name,
    desc,
    is_limit,
    latest_chapter_name,
    latest_chapter_id,
    word_count,
    read_count,
    reply_count,
    author_id,
    author_name
FROM novel;

DROP TABLE novel;
ALTER TABLE novel_new RENAME TO novel;
