use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RankingFormat {
    Pdf,
    Epub,
    Mobi,
    Azw3,
    Djvu,
    Cbz,
    Cbr,
    Txt,
    Rtf,
    Doc,
    Docx,
    Zip,
    Unknown(String),
}

impl RankingFormat {
    pub fn parse(value: &str) -> Self {
        let normalized = normalize_token(value);

        match normalized.as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "mobi" => Self::Mobi,
            "azw3" => Self::Azw3,
            "djvu" => Self::Djvu,
            "cbz" => Self::Cbz,
            "cbr" => Self::Cbr,
            "txt" => Self::Txt,
            "rtf" => Self::Rtf,
            "doc" => Self::Doc,
            "docx" => Self::Docx,
            "zip" => Self::Zip,
            _ => Self::Unknown(normalized),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Mobi => "mobi",
            Self::Azw3 => "azw3",
            Self::Djvu => "djvu",
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Txt => "txt",
            Self::Rtf => "rtf",
            Self::Doc => "doc",
            Self::Docx => "docx",
            Self::Zip => "zip",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RankingCategory {
    Fiction,
    Nonfiction,
    Comic,
    Magazine,
    MusicalScore,
    Other,
    Unknown(String),
}

impl RankingCategory {
    pub fn parse(value: &str) -> Self {
        let normalized = normalize_token(value);

        match normalized.as_str() {
            "fiction" => Self::Fiction,
            "nonfiction" => Self::Nonfiction,
            "comic" => Self::Comic,
            "magazine" => Self::Magazine,
            "musicalscore" | "musical_score" | "musical-score" => Self::MusicalScore,
            "other" => Self::Other,
            _ => Self::Unknown(normalized),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingPreferences {
    pub preferred_formats: Vec<RankingFormat>,
    pub preferred_languages: Vec<String>,
    pub preferred_categories: Vec<RankingCategory>,
    pub preferred_year: Option<u16>,
    pub minimum_year: Option<u16>,
    pub maximum_year: Option<u16>,
    pub preferred_extension_weight: f32,
    pub preferred_language_weight: f32,
    pub preferred_author_weight: f32,
    pub exact_title_weight: f32,
    pub fuzzy_title_weight: f32,
    pub file_size_weight: f32,
    pub metadata_completeness_weight: f32,
    pub source_reputation_weight: f32,
    pub year_match_weight: f32,
    pub category_match_weight: f32,
    pub mismatch_penalty_weight: f32,
    pub non_downloadable_penalty_weight: f32,
}

impl Default for RankingPreferences {
    fn default() -> Self {
        Self {
            preferred_formats: vec![
                RankingFormat::Epub,
                RankingFormat::Pdf,
                RankingFormat::Mobi,
                RankingFormat::Azw3,
            ],
            preferred_languages: vec!["en".to_string(), "english".to_string()],
            preferred_categories: Vec::new(),
            preferred_year: None,
            minimum_year: None,
            maximum_year: None,
            preferred_extension_weight: 12.0,
            preferred_language_weight: 10.0,
            preferred_author_weight: 14.0,
            exact_title_weight: 28.0,
            fuzzy_title_weight: 22.0,
            file_size_weight: 7.0,
            metadata_completeness_weight: 6.0,
            source_reputation_weight: 5.0,
            year_match_weight: 6.0,
            category_match_weight: 4.0,
            mismatch_penalty_weight: 18.0,
            non_downloadable_penalty_weight: 35.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RankingQuery {
    pub raw_query: String,
    pub title_hint: Option<String>,
    pub author_hint: Option<String>,
}

impl RankingQuery {
    pub fn normalized_query(&self) -> String {
        normalize_text(&self.raw_query)
    }

    pub fn normalized_title_hint(&self) -> Option<String> {
        self.title_hint.as_ref().map(|value| normalize_text(value))
    }

    pub fn normalized_author_hint(&self) -> Option<String> {
        self.author_hint.as_ref().map(|value| normalize_text(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RankingCandidate {
    pub result_id: String,
    pub provider: String,
    pub provider_item_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub year: Option<u16>,
    pub language: Option<String>,
    pub format: Option<String>,
    pub category: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub source_label: Option<String>,
    pub checksum_md5: Option<String>,
    pub downloadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingScoreBreakdown {
    pub score: f32,
    pub exact_title_score: f32,
    pub fuzzy_title_score: f32,
    pub author_score: f32,
    pub language_score: f32,
    pub format_score: f32,
    pub year_score: f32,
    pub file_size_score: f32,
    pub metadata_score: f32,
    pub category_score: f32,
    pub source_score: f32,
    pub mismatch_penalty: f32,
    pub non_downloadable_penalty: f32,
    pub reasons: Vec<String>,
}

impl Default for RankingScoreBreakdown {
    fn default() -> Self {
        Self {
            score: 0.0,
            exact_title_score: 0.0,
            fuzzy_title_score: 0.0,
            author_score: 0.0,
            language_score: 0.0,
            format_score: 0.0,
            year_score: 0.0,
            file_size_score: 0.0,
            metadata_score: 0.0,
            category_score: 0.0,
            source_score: 0.0,
            mismatch_penalty: 0.0,
            non_downloadable_penalty: 0.0,
            reasons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCandidate {
    pub candidate: RankingCandidate,
    pub breakdown: RankingScoreBreakdown,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionDecision {
    pub selected: Option<RankedCandidate>,
    pub alternatives: Vec<RankedCandidate>,
    pub confident: bool,
    pub confidence_margin: Option<f32>,
    pub reason: String,
}

pub fn rank_candidates(
    query: &RankingQuery,
    candidates: impl IntoIterator<Item = RankingCandidate>,
    preferences: &RankingPreferences,
) -> Vec<RankedCandidate> {
    let mut ranked: Vec<RankedCandidate> = candidates
        .into_iter()
        .map(|candidate| RankedCandidate {
            breakdown: score_candidate(query, &candidate, preferences),
            candidate,
            rank: 0,
        })
        .collect();

    ranked.sort_by(compare_ranked_candidates);

    for (index, ranked_candidate) in ranked.iter_mut().enumerate() {
        ranked_candidate.rank = index + 1;
    }

    ranked
}

pub fn select_best_candidate(
    query: &RankingQuery,
    candidates: impl IntoIterator<Item = RankingCandidate>,
    preferences: &RankingPreferences,
    minimum_confidence_margin: f32,
    minimum_absolute_score: f32,
) -> SelectionDecision {
    let ranked = rank_candidates(query, candidates, preferences);

    if ranked.is_empty() {
        return SelectionDecision {
            selected: None,
            alternatives: Vec::new(),
            confident: false,
            confidence_margin: None,
            reason: "No candidates available to rank.".to_string(),
        };
    }

    let best = ranked[0].clone();
    let second = ranked.get(1).cloned();

    let margin = second
        .as_ref()
        .map(|next| best.breakdown.score - next.breakdown.score);

    let confident = best.breakdown.score >= minimum_absolute_score
        && margin.unwrap_or(best.breakdown.score) >= minimum_confidence_margin;

    let reason = if confident {
        match margin {
            Some(value) => format!(
                "Selected top-ranked candidate with score {:.2} and confidence margin {:.2}.",
                best.breakdown.score, value
            ),
            None => format!(
                "Selected only available candidate with score {:.2}.",
                best.breakdown.score
            ),
        }
    } else if best.breakdown.score < minimum_absolute_score {
        format!(
            "Top result score {:.2} is below the minimum required score {:.2}.",
            best.breakdown.score, minimum_absolute_score
        )
    } else {
        format!(
            "Top result did not exceed the required confidence margin of {:.2}.",
            minimum_confidence_margin
        )
    };

    SelectionDecision {
        selected: confident.then_some(best),
        alternatives: ranked,
        confident,
        confidence_margin: margin,
        reason,
    }
}

pub fn score_candidate(
    query: &RankingQuery,
    candidate: &RankingCandidate,
    preferences: &RankingPreferences,
) -> RankingScoreBreakdown {
    let mut breakdown = RankingScoreBreakdown::default();

    let normalized_query = query.normalized_query();
    let normalized_title_hint = query.normalized_title_hint();
    let normalized_author_hint = query.normalized_author_hint();

    let title = normalize_text(&candidate.title);
    let subtitle = candidate
        .subtitle
        .as_ref()
        .map(|value| normalize_text(value))
        .unwrap_or_default();
    let title_blob = if subtitle.is_empty() {
        title.clone()
    } else {
        format!("{title} {subtitle}")
    };

    let author_blob = candidate
        .authors
        .iter()
        .map(|value| normalize_text(value))
        .collect::<Vec<_>>()
        .join(" ");

    let language = candidate
        .language
        .as_deref()
        .map(normalize_token)
        .unwrap_or_default();

    let format = candidate.format.as_deref().map(RankingFormat::parse);

    let category = candidate.category.as_deref().map(RankingCategory::parse);

    let query_tokens = tokenize(&normalized_query);
    let title_tokens = tokenize(&title_blob);
    let author_tokens = tokenize(&author_blob);

    let exact_title_score = exact_title_match_score(
        &normalized_query,
        normalized_title_hint.as_deref(),
        &title,
        &title_blob,
        preferences.exact_title_weight,
    );
    if exact_title_score > 0.0 {
        breakdown.reasons.push("Strong title match.".to_string());
    }
    breakdown.exact_title_score = exact_title_score;

    let fuzzy_title_similarity = best_similarity(
        &[
            normalized_query.as_str(),
            normalized_title_hint.as_deref().unwrap_or(""),
        ],
        &[title.as_str(), title_blob.as_str()],
    );
    breakdown.fuzzy_title_score = fuzzy_title_similarity * preferences.fuzzy_title_weight;
    if breakdown.fuzzy_title_score > 0.0 {
        breakdown.reasons.push(format!(
            "Fuzzy title similarity contributed {:.2}.",
            breakdown.fuzzy_title_score
        ));
    }

    let author_query = normalized_author_hint
        .as_deref()
        .unwrap_or_else(|| infer_author_fragment(&normalized_query).unwrap_or(""));
    let author_score = if author_query.is_empty() {
        0.0
    } else {
        let score = best_similarity(&[author_query], &[author_blob.as_str()]);
        score * preferences.preferred_author_weight
    };
    breakdown.author_score = author_score;
    if author_score > 0.0 {
        breakdown.reasons.push(format!(
            "Author similarity contributed {:.2}.",
            author_score
        ));
    }

    let language_score = score_language(&language, preferences);
    breakdown.language_score = language_score;
    if language_score > 0.0 {
        breakdown.reasons.push(format!(
            "Preferred language match contributed {:.2}.",
            language_score
        ));
    }

    let format_score = score_format(format.as_ref(), preferences);
    breakdown.format_score = format_score;
    if format_score > 0.0 {
        breakdown.reasons.push(format!(
            "Preferred format match contributed {:.2}.",
            format_score
        ));
    }

    let year_score = score_year(candidate.year, preferences);
    breakdown.year_score = year_score;
    if year_score > 0.0 {
        breakdown
            .reasons
            .push(format!("Year preference contributed {:.2}.", year_score));
    }

    let file_size_score = score_file_size(candidate.file_size_bytes, format.as_ref(), preferences);
    breakdown.file_size_score = file_size_score;
    if file_size_score > 0.0 {
        breakdown.reasons.push(format!(
            "Plausible file size contributed {:.2}.",
            file_size_score
        ));
    }

    let metadata_score = score_metadata_completeness(candidate, preferences);
    breakdown.metadata_score = metadata_score;
    if metadata_score > 0.0 {
        breakdown.reasons.push(format!(
            "Metadata completeness contributed {:.2}.",
            metadata_score
        ));
    }

    let category_score = score_category(category.as_ref(), preferences);
    breakdown.category_score = category_score;
    if category_score > 0.0 {
        breakdown.reasons.push(format!(
            "Preferred category contributed {:.2}.",
            category_score
        ));
    }

    let source_score = score_source(
        candidate.source_label.as_deref(),
        candidate.provider.as_str(),
        preferences,
    );
    breakdown.source_score = source_score;
    if source_score > 0.0 {
        breakdown.reasons.push(format!(
            "Source reputation contributed {:.2}.",
            source_score
        ));
    }

    let mismatch_penalty = score_mismatch_penalty(
        &query_tokens,
        &title_tokens,
        &author_tokens,
        candidate,
        preferences,
    );
    breakdown.mismatch_penalty = mismatch_penalty;
    if mismatch_penalty > 0.0 {
        breakdown.reasons.push(format!(
            "Mismatch penalties reduced the score by {:.2}.",
            mismatch_penalty
        ));
    }

    if !candidate.downloadable {
        breakdown.non_downloadable_penalty = preferences.non_downloadable_penalty_weight;
        breakdown
            .reasons
            .push("Candidate is not currently downloadable.".to_string());
    }

    breakdown.score = breakdown.exact_title_score
        + breakdown.fuzzy_title_score
        + breakdown.author_score
        + breakdown.language_score
        + breakdown.format_score
        + breakdown.year_score
        + breakdown.file_size_score
        + breakdown.metadata_score
        + breakdown.category_score
        + breakdown.source_score
        - breakdown.mismatch_penalty
        - breakdown.non_downloadable_penalty;

    breakdown
}

fn compare_ranked_candidates(left: &RankedCandidate, right: &RankedCandidate) -> Ordering {
    right
        .breakdown
        .score
        .partial_cmp(&left.breakdown.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            let left_downloadable = left.candidate.downloadable;
            let right_downloadable = right.candidate.downloadable;
            right_downloadable.cmp(&left_downloadable)
        })
        .then_with(|| {
            right
                .candidate
                .file_size_bytes
                .unwrap_or(0)
                .cmp(&left.candidate.file_size_bytes.unwrap_or(0))
        })
        .then_with(|| left.candidate.title.cmp(&right.candidate.title))
        .then_with(|| left.candidate.result_id.cmp(&right.candidate.result_id))
}

fn exact_title_match_score(
    normalized_query: &str,
    normalized_title_hint: Option<&str>,
    title: &str,
    title_blob: &str,
    max_weight: f32,
) -> f32 {
    let candidates = [normalized_query, normalized_title_hint.unwrap_or_default()];

    for query in candidates {
        if query.is_empty() {
            continue;
        }

        if query == title || query == title_blob {
            return max_weight;
        }

        if title.starts_with(query) || title_blob.starts_with(query) {
            return max_weight * 0.92;
        }

        if title.contains(query) || title_blob.contains(query) {
            return max_weight * 0.84;
        }
    }

    0.0
}

fn score_language(language: &str, preferences: &RankingPreferences) -> f32 {
    if language.is_empty() || preferences.preferred_languages.is_empty() {
        return 0.0;
    }

    let preferred: Vec<String> = preferences
        .preferred_languages
        .iter()
        .map(|value| normalize_token(value))
        .collect();

    if preferred.iter().any(|value| value == language) {
        return preferences.preferred_language_weight;
    }

    if preferred
        .iter()
        .any(|value| value.starts_with(language) || language.starts_with(value))
    {
        return preferences.preferred_language_weight * 0.75;
    }

    0.0
}

fn score_format(format: Option<&RankingFormat>, preferences: &RankingPreferences) -> f32 {
    let Some(format) = format else {
        return 0.0;
    };

    if preferences.preferred_formats.is_empty() {
        return 0.0;
    }

    for (index, preferred) in preferences.preferred_formats.iter().enumerate() {
        if preferred == format {
            let divisor = (index as f32) + 1.0;
            return preferences.preferred_extension_weight / divisor;
        }
    }

    0.0
}

fn score_year(year: Option<u16>, preferences: &RankingPreferences) -> f32 {
    let Some(year) = year else {
        return 0.0;
    };

    if let Some(min_year) = preferences.minimum_year {
        if year < min_year {
            return 0.0;
        }
    }

    if let Some(max_year) = preferences.maximum_year {
        if year > max_year {
            return 0.0;
        }
    }

    if let Some(preferred_year) = preferences.preferred_year {
        let diff = preferred_year.abs_diff(year) as f32;
        let ratio = (1.0 - (diff / 20.0)).clamp(0.0, 1.0);
        return ratio * preferences.year_match_weight;
    }

    preferences.year_match_weight * 0.5
}

fn score_file_size(
    file_size_bytes: Option<u64>,
    format: Option<&RankingFormat>,
    preferences: &RankingPreferences,
) -> f32 {
    let Some(size) = file_size_bytes else {
        return 0.0;
    };

    let mb = size as f32 / (1024.0 * 1024.0);
    let ideal = match format {
        Some(RankingFormat::Epub) => 3.0,
        Some(RankingFormat::Pdf) => 15.0,
        Some(RankingFormat::Mobi) => 4.0,
        Some(RankingFormat::Azw3) => 5.0,
        Some(RankingFormat::Djvu) => 10.0,
        Some(RankingFormat::Cbz | RankingFormat::Cbr) => 40.0,
        _ => 8.0,
    };

    if mb <= 0.0 {
        return 0.0;
    }

    let distance_ratio = ((mb - ideal).abs() / ideal.max(1.0)).min(2.0);
    let normalized = (1.0 - (distance_ratio / 2.0)).clamp(0.0, 1.0);
    normalized * preferences.file_size_weight
}

fn score_metadata_completeness(
    candidate: &RankingCandidate,
    preferences: &RankingPreferences,
) -> f32 {
    let mut present = 0.0;
    let mut total = 0.0;

    let fields = [
        !candidate.title.trim().is_empty(),
        !candidate.authors.is_empty(),
        candidate.year.is_some(),
        candidate.language.is_some(),
        candidate.format.is_some(),
        candidate.file_size_bytes.is_some(),
        candidate.publisher.is_some(),
        candidate.checksum_md5.is_some(),
        candidate.source_label.is_some(),
    ];

    for present_field in fields {
        total += 1.0;
        if present_field {
            present += 1.0;
        }
    }

    if total == 0.0 {
        return 0.0;
    }

    (present / total) * preferences.metadata_completeness_weight
}

fn score_category(category: Option<&RankingCategory>, preferences: &RankingPreferences) -> f32 {
    let Some(category) = category else {
        return 0.0;
    };

    if preferences.preferred_categories.is_empty() {
        return 0.0;
    }

    if preferences
        .preferred_categories
        .iter()
        .any(|value| value == category)
    {
        preferences.category_match_weight
    } else {
        0.0
    }
}

fn score_source(
    source_label: Option<&str>,
    provider: &str,
    preferences: &RankingPreferences,
) -> f32 {
    let mut weights: HashMap<&str, f32> = HashMap::new();
    weights.insert("annas archive", 1.0);
    weights.insert("libgenrs", 0.92);
    weights.insert("libgenli", 0.9);
    weights.insert("zlibrary", 0.86);
    weights.insert("other", 0.5);

    let normalized_source = source_label
        .map(normalize_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| normalize_text(provider));

    let multiplier = weights
        .iter()
        .find_map(|(key, value)| normalized_source.contains(key).then_some(*value))
        .unwrap_or(0.65);

    multiplier * preferences.source_reputation_weight
}

fn score_mismatch_penalty(
    query_tokens: &HashSet<String>,
    title_tokens: &HashSet<String>,
    author_tokens: &HashSet<String>,
    candidate: &RankingCandidate,
    preferences: &RankingPreferences,
) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let matched_tokens = query_tokens
        .iter()
        .filter(|token| title_tokens.contains(*token) || author_tokens.contains(*token))
        .count();

    let coverage = matched_tokens as f32 / query_tokens.len() as f32;
    let mut penalty = (1.0 - coverage).clamp(0.0, 1.0) * preferences.mismatch_penalty_weight;

    if let Some(format) = candidate.format.as_deref() {
        let normalized = normalize_token(format);
        if normalized == "zip" {
            penalty += preferences.mismatch_penalty_weight * 0.2;
        }
    }

    if candidate.title.to_lowercase().contains("summary")
        || candidate.title.to_lowercase().contains("workbook")
        || candidate.title.to_lowercase().contains("study guide")
    {
        penalty += preferences.mismatch_penalty_weight * 0.35;
    }

    penalty
}

fn best_similarity(lefts: &[&str], rights: &[&str]) -> f32 {
    let mut best = 0.0_f32;

    for left in lefts {
        if left.trim().is_empty() {
            continue;
        }

        for right in rights {
            if right.trim().is_empty() {
                continue;
            }

            best = best.max(jaccard_similarity(left, right));
            best = best.max(ordered_token_overlap(left, right));
        }
    }

    best.clamp(0.0, 1.0)
}

fn jaccard_similarity(left: &str, right: &str) -> f32 {
    let left_tokens = tokenize(left);
    let right_tokens = tokenize(right);

    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let intersection = left_tokens.intersection(&right_tokens).count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn ordered_token_overlap(left: &str, right: &str) -> f32 {
    let left_tokens: Vec<String> = tokenize(left).into_iter().collect();
    let right_tokens: Vec<String> = tokenize(right).into_iter().collect();

    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let right_index: HashMap<&str, usize> = right_tokens
        .iter()
        .enumerate()
        .map(|(index, token)| (token.as_str(), index))
        .collect();

    let mut matched = 0usize;
    let mut last_index = None;

    for token in &left_tokens {
        let Some(index) = right_index.get(token.as_str()).copied() else {
            continue;
        };

        if last_index.map(|value| index >= value).unwrap_or(true) {
            matched += 1;
            last_index = Some(index);
        }
    }

    matched as f32 / left_tokens.len().max(right_tokens.len()) as f32
}

fn tokenize(value: &str) -> HashSet<String> {
    normalize_text(value)
        .split_whitespace()
        .filter(|token| token.len() > 1)
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_token(value: &str) -> String {
    normalize_text(value).replace(' ', "")
}

fn infer_author_fragment(query: &str) -> Option<&str> {
    query
        .split(" by ")
        .nth(1)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs() -> RankingPreferences {
        RankingPreferences::default()
    }

    fn query(raw: &str) -> RankingQuery {
        RankingQuery {
            raw_query: raw.to_string(),
            title_hint: None,
            author_hint: None,
        }
    }

    fn candidate(
        id: &str,
        title: &str,
        author: &str,
        format: &str,
        language: &str,
        downloadable: bool,
    ) -> RankingCandidate {
        RankingCandidate {
            result_id: id.to_string(),
            provider: "annas_archive".to_string(),
            provider_item_id: format!("provider-{id}"),
            title: title.to_string(),
            subtitle: None,
            authors: vec![author.to_string()],
            publisher: Some("Test Publisher".to_string()),
            year: Some(2021),
            language: Some(language.to_string()),
            format: Some(format.to_string()),
            category: Some("nonfiction".to_string()),
            file_size_bytes: Some(4 * 1024 * 1024),
            source_label: Some("libgenRs".to_string()),
            checksum_md5: Some("abc123".to_string()),
            downloadable,
        }
    }

    #[test]
    fn prefers_exact_title_and_author_match() {
        let query = RankingQuery {
            raw_query: "Clean Code Robert Martin".to_string(),
            title_hint: Some("Clean Code".to_string()),
            author_hint: Some("Robert Martin".to_string()),
        };

        let good = candidate("1", "Clean Code", "Robert Martin", "epub", "en", true);
        let bad = candidate("2", "The Clean Coder", "Robert Martin", "epub", "en", true);

        let ranked = rank_candidates(&query, vec![bad.clone(), good.clone()], &prefs());

        assert_eq!(ranked[0].candidate.result_id, "1");
        assert!(ranked[0].breakdown.score > ranked[1].breakdown.score);
    }

    #[test]
    fn penalizes_non_downloadable_candidates() {
        let query = query("Dune");

        let downloadable = candidate("1", "Dune", "Frank Herbert", "epub", "en", true);
        let unavailable = candidate("2", "Dune", "Frank Herbert", "epub", "en", false);

        let ranked = rank_candidates(&query, vec![unavailable, downloadable], &prefs());

        assert_eq!(ranked[0].candidate.result_id, "1");
    }

    #[test]
    fn prefers_requested_format_order() {
        let mut preferences = prefs();
        preferences.preferred_formats = vec![RankingFormat::Pdf, RankingFormat::Epub];

        let query = query("Sapiens");
        let pdf = candidate("1", "Sapiens", "Yuval Noah Harari", "pdf", "en", true);
        let epub = candidate("2", "Sapiens", "Yuval Noah Harari", "epub", "en", true);

        let ranked = rank_candidates(&query, vec![epub, pdf], &preferences);

        assert_eq!(ranked[0].candidate.result_id, "1");
    }

    #[test]
    fn returns_unconfident_when_margin_is_small() {
        let query = query("Atomic Habits");
        let first = candidate("1", "Atomic Habits", "James Clear", "epub", "en", true);
        let second = candidate(
            "2",
            "Atomic Habits Workbook",
            "James Clear",
            "pdf",
            "en",
            true,
        );

        let decision = select_best_candidate(&query, vec![first, second], &prefs(), 50.0, 15.0);

        assert!(!decision.confident);
        assert!(decision.selected.is_none());
        assert_eq!(decision.alternatives.len(), 2);
    }

    #[test]
    fn tokenize_normalizes_punctuation() {
        let tokens = tokenize("The Rust-Programming Language, 2nd Edition!");

        assert!(tokens.contains("rust"));
        assert!(tokens.contains("programming"));
        assert!(tokens.contains("language"));
        assert!(tokens.contains("2nd"));
        assert!(tokens.contains("edition"));
    }
}
