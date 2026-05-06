/*
 * @Author: suxiaoshao suxiaoshao@gmail.com
 * @Date: 2024-01-06 01:08:42
 * @LastEditors: suxiaoshao suxiaoshao@gmail.com
 * @LastEditTime: 2024-01-07 19:25:25
 * @FilePath: /tauri/packages/feiwen/src-tauri/src/fetch/parse_novel/parse_count.rs
 */
use std::sync::LazyLock;

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    combinator::{opt, value},
    number::complete::float,
};
use scraper::{Html, Selector};

use crate::{
    errors::{FeiwenError, FeiwenResult},
    store::types::NovelCount,
};
static SELECTOR_COUNT: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("div.col-xs-12.h5.brief-0 > span.pull-right.smaller-30 > em").unwrap()
});

pub(crate) fn parse_count(doc: &Html) -> FeiwenResult<NovelCount> {
    let count = doc
        .select(&SELECTOR_COUNT)
        .next()
        .ok_or(FeiwenError::CountParse)?;
    let count = count.text().collect::<String>();
    let count = count
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let word_count = count.first().ok_or(FeiwenError::WordCountParse)?;
    let word_count = parse_num_with_unit(word_count)?;
    let read_count = count
        .get(1)
        .map(|read_count| parse_num_with_unit(read_count))
        .transpose()?;
    let reply_count = count
        .get(2)
        .map(|reply_count| parse_num_with_unit(reply_count))
        .transpose()?;
    Ok(NovelCount {
        word_count,
        read_count,
        reply_count,
    })
}
fn parse_num_with_unit(num: &str) -> FeiwenResult<i32> {
    fn inner_parse(num: &str) -> IResult<&str, i32> {
        #[derive(Clone, Copy)]
        enum Flag {
            Qian,
            Wan,
            K,
        }
        let (input, (num, flag)) = (
            float,
            opt(alt((
                value(Flag::Qian, tag("千")),
                value(Flag::Wan, tag("万")),
                value(Flag::K, tag("k")),
                value(Flag::K, tag("K")),
            ))),
        )
            .parse(num)?;
        let num = match flag {
            Some(Flag::Qian) => num * 1000f32,
            Some(Flag::Wan) => num * 10000f32,
            Some(Flag::K) => num * 1000f32,
            None => num,
        };
        Ok((input, num as i32))
    }
    let num = num.trim().replace(',', "");
    let (_, num) = inner_parse(&num).map_err(|err| FeiwenError::CountUintParse(err.to_string()))?;
    Ok(num)
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use super::{parse_count, parse_num_with_unit};

    #[test]
    fn parses_current_books_count_with_only_word_count() {
        let doc = Html::parse_document(
            r#"
            <div class="col-xs-12 h5 brief-0">
                <span class="smaller-5">简介</span>
                <span class = "pull-right smaller-30"><em><span class="glyphicon glyphicon-pencil"></span>16万</em></span>
            </div>
            "#,
        );

        let count = parse_count(&doc).unwrap();
        assert_eq!(count.word_count, 160000);
        assert_eq!(count.read_count, None);
        assert_eq!(count.reply_count, None);
    }

    #[test]
    fn parses_legacy_books_count_when_all_counts_are_present() {
        let doc = Html::parse_document(
            r#"
            <div class="col-xs-12 h5 brief-0">
                <span class = "pull-right smaller-30"><em>16万 / 2.5万 / 30</em></span>
            </div>
            "#,
        );

        let count = parse_count(&doc).unwrap();
        assert_eq!(count.word_count, 160000);
        assert_eq!(count.read_count, Some(25000));
        assert_eq!(count.reply_count, Some(30));
    }

    #[test]
    fn parses_k_suffix() {
        assert_eq!(parse_num_with_unit("63k").unwrap(), 63000);
        assert_eq!(parse_num_with_unit("1.5K").unwrap(), 1500);
    }
}
