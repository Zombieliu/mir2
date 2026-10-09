//! Pure Source CHECKHUM selection and comparison, not shared-world authority.
//!
//! Crystal creates maps in MapInfoList order (Envir.cs:3347-3350) and appends a
//! map only after Load succeeds (MapInfo.cs:206-210). GetMapByNameAndInstance
//! matches FileName and selects among those successful entries, not MapInfo
//! index, title, current player map, normalized path, AOI, or a ranking roster
//! (Envir.cs:4613-4619). The caller must supply that actual loaded directory.
//!
//! Population must come from the complete authoritative map Players collection,
//! including dead players until RemoveObject (Map.cs:2361-2377). This module
//! neither loads maps nor verifies owner leases, online Nodes, life/generation,
//! transport presence, Source sequence, NPC invocation, or map initialization.
//! A constructor name is not proof. None of the directory, count, or check types
//! can be deserialized into live authority; they are crate-private pure data.
//!
//! Source uses CurrentCultureIgnoreCase for FileName. This bounded adapter only
//! supports the ASCII FileName domain of the imported directory. It preserves
//! spelling and whitespace and never creates trim, suffix, or title aliases.
//! Non-ASCII directories are rejected and non-ASCII queries are unavailable,
//! rather than claiming a culture comparison this module does not implement.

use std::fmt;

/// One successfully loaded Source MapInfo, retaining its original position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcLoadedSourceMap {
    source_ordinal: usize,
    source_index: i32,
    file_name: String,
}

impl NpcLoadedSourceMap {
    /// The trusted boot producer, not this method, proves successful loading.
    pub(crate) fn from_successful_load(
        source_ordinal: usize,
        source_index: i32,
        file_name: String,
    ) -> Result<Self, NpcPopulationCatalogError> {
        if !file_name.is_ascii() {
            return Err(NpcPopulationCatalogError::UnsupportedNonAsciiFileName);
        }
        Ok(Self {
            source_ordinal,
            source_index,
            file_name,
        })
    }

    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn source_index(&self) -> i32 {
        self.source_index
    }

    pub(crate) fn file_name(&self) -> &str {
        &self.file_name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcPopulationCatalogError {
    SourceOrderNotStrictlyIncreasing,
    UnsupportedNonAsciiFileName,
}

impl fmt::Display for NpcPopulationCatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceOrderNotStrictlyIncreasing => {
                "loaded NPC maps are not in strictly increasing Source order"
            }
            Self::UnsupportedNonAsciiFileName => {
                "loaded NPC map FileName requires an unsupported culture comparison"
            }
        })
    }
}

impl std::error::Error for NpcPopulationCatalogError {}

/// Immutable successful entries in original MapInfo order, with failed loads
/// absent. The trusted producer must provide a complete loaded directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcLoadedMapCatalog {
    maps: Vec<NpcLoadedSourceMap>,
}

impl NpcLoadedMapCatalog {
    pub(crate) fn from_successful_source_order(
        maps: Vec<NpcLoadedSourceMap>,
    ) -> Result<Self, NpcPopulationCatalogError> {
        if maps
            .windows(2)
            .any(|pair| pair[0].source_ordinal >= pair[1].source_ordinal)
        {
            return Err(NpcPopulationCatalogError::SourceOrderNotStrictlyIncreasing);
        }
        Ok(Self { maps })
    }

    pub(crate) fn maps(&self) -> &[NpcLoadedSourceMap] {
        &self.maps
    }

    /// The resolver may read an existing real Zone, or return known zero for an
    /// empty loaded map whose ownership is actually established. This method
    /// does not manufacture a Zone or reserve admission. None means unavailable
    /// authority; it never means zero players or an absent Source map.
    pub(crate) fn query(
        &self,
        file_name: &str,
        source_instance: i32,
        read_count: impl FnOnce(&NpcLoadedSourceMap) -> Option<NpcPopulationCount>,
    ) -> NpcPopulationQuery {
        if !file_name.is_ascii() {
            return NpcPopulationQuery::AuthorityUnavailable;
        }

        // Envir.cs:4615-4616: negatives become zero; positive values decrement.
        // Thus negative, zero, and one all select the first successful instance.
        let instance_ordinal = if source_instance > 0 {
            source_instance - 1
        } else {
            0
        };
        let Ok(instance_ordinal) = usize::try_from(instance_ordinal) else {
            return NpcPopulationQuery::MissingSourceMap;
        };
        let Some(map) = self
            .maps
            .iter()
            .filter(|map| map.file_name.eq_ignore_ascii_case(file_name))
            .nth(instance_ordinal)
        else {
            return NpcPopulationQuery::MissingSourceMap;
        };

        match read_count(map) {
            Some(count) => NpcPopulationQuery::KnownCount(count),
            None => NpcPopulationQuery::AuthorityUnavailable,
        }
    }
}

/// A representable Players.Count value, not a token granting online authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NpcPopulationCount(i32);

impl NpcPopulationCount {
    pub(crate) fn try_from_usize(count: usize) -> Option<Self> {
        i32::try_from(count).ok().map(Self)
    }

    pub(crate) fn get(self) -> i32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcPopulationQuery {
    KnownCount(NpcPopulationCount),
    MissingSourceMap,
    AuthorityUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcPopulationError {
    AuthorityUnavailable,
    InvalidComparisonOperator,
}

impl fmt::Display for NpcPopulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AuthorityUnavailable => "NPC map population authority is unavailable",
            Self::InvalidComparisonOperator => "invalid Source CHECKHUM comparison operator",
        })
    }
}

impl std::error::Error for NpcPopulationError {}

/// Source-parsed arguments after the caller's independent NPC substitution.
/// Do not join/re-split values: a substituted value stays one argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpcCheckHum {
    operator: String,
    expected_count: String,
    file_name: String,
    source_instance: String,
}

impl NpcCheckHum {
    /// NPCSegment.cs:255-259 omits an incomplete condition, defaults instance
    /// to "1", and ignores arguments beyond the explicit instance. Returning
    /// None is that parser omission, not a condition that evaluates to false.
    /// The sealed upstream parser must perform omission before substitutions.
    pub(crate) fn parse_source_arguments(arguments: &[&str]) -> Option<Self> {
        let [operator, expected_count, file_name, rest @ ..] = arguments else {
            return None;
        };
        Some(Self {
            operator: (*operator).to_owned(),
            expected_count: (*expected_count).to_owned(),
            file_name: (*file_name).to_owned(),
            source_instance: rest.first().copied().unwrap_or("1").to_owned(),
        })
    }

    pub(crate) fn file_name(&self) -> &str {
        &self.file_name
    }

    /// NPCSegment.cs:2351 short-circuits count parsing before instance parsing;
    /// 2357-2362 then resolves the map; only 2364 calls Compare with its count.
    /// An invalid operator therefore throws only after a real map count is read
    /// (4996-5006). Unavailable shared authority is an integration error, never
    /// the Source false branch or fabricated count zero.
    pub(crate) fn evaluate(
        &self,
        catalog: &NpcLoadedMapCatalog,
        read_count: impl FnOnce(&NpcLoadedSourceMap) -> Option<NpcPopulationCount>,
    ) -> Result<bool, NpcPopulationError> {
        let Some(expected_count) = try_parse_source_i32(&self.expected_count) else {
            return Ok(false);
        };
        let Some(source_instance) = try_parse_source_i32(&self.source_instance) else {
            return Ok(false);
        };
        let count = match catalog.query(&self.file_name, source_instance, read_count) {
            NpcPopulationQuery::KnownCount(count) => count.get(),
            NpcPopulationQuery::MissingSourceMap => return Ok(false),
            NpcPopulationQuery::AuthorityUnavailable => {
                return Err(NpcPopulationError::AuthorityUnavailable);
            }
        };

        match self.operator.as_str() {
            "<" => Ok(count < expected_count),
            ">" => Ok(count > expected_count),
            "<=" => Ok(count <= expected_count),
            ">=" => Ok(count >= expected_count),
            "==" => Ok(count == expected_count),
            "!=" => Ok(count != expected_count),
            _ => Err(NpcPopulationError::InvalidComparisonOperator),
        }
    }
}

/// Int32.TryParse's ordinary Source integer contract: ASCII digits, optional
/// ASCII leading sign, Integer-style ASCII outer whitespace, and checked i32
/// range. Trailing NUL compatibility is retained, not treated as inner space.
/// Custom culture sign strings are outside this bounded Source adapter.
fn try_parse_source_i32(value: &str) -> Option<i32> {
    let value = value
        .trim_end_matches('\0')
        .trim_matches(|ch: char| matches!(ch, '\u{0009}'..='\u{000d}' | ' '));
    let digits = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // Isolated pure inputs, not claims that any real map was loaded or owned.
    fn map(ordinal: usize, index: i32, file_name: &str) -> NpcLoadedSourceMap {
        NpcLoadedSourceMap::from_successful_load(ordinal, index, file_name.to_owned()).unwrap()
    }

    fn catalog() -> NpcLoadedMapCatalog {
        // Source ordinals 1 and 3 represent failed loads and are absent. Reversed
        // indices ensure instance order is not silently changed to index order.
        NpcLoadedMapCatalog::from_successful_source_order(vec![
            map(0, 900, "EM001"),
            map(2, 5, "OTHER"),
            map(4, 7, "em001"),
            map(8, 7, "EM001"),
        ])
        .unwrap()
    }

    fn count(value: usize) -> NpcPopulationCount {
        NpcPopulationCount::try_from_usize(value).unwrap()
    }

    fn check(arguments: &[&str]) -> NpcCheckHum {
        NpcCheckHum::parse_source_arguments(arguments).unwrap()
    }

    #[test]
    fn retains_successful_source_ordinals_indices_and_exact_file_names() {
        let catalog = catalog();
        assert_eq!(catalog.maps().len(), 4);
        assert_eq!(catalog.maps()[0].source_ordinal(), 0);
        assert_eq!(catalog.maps()[0].source_index(), 900);
        assert_eq!(catalog.maps()[1].source_ordinal(), 2);
        assert_eq!(catalog.maps()[2].source_index(), 7);
        assert_eq!(catalog.maps()[2].file_name(), "em001");
    }

    #[test]
    fn never_sorts_out_of_order_or_duplicate_source_ordinals() {
        for maps in [
            vec![map(4, 7, "EM001"), map(0, 900, "EM001")],
            vec![map(0, 900, "EM001"), map(0, 7, "EM001")],
        ] {
            assert_eq!(
                NpcLoadedMapCatalog::from_successful_source_order(maps),
                Err(NpcPopulationCatalogError::SourceOrderNotStrictlyIncreasing)
            );
        }
    }

    #[test]
    fn same_name_instances_follow_successful_order_not_source_index() {
        let catalog = catalog();
        for (instance, expected_ordinal, expected_index) in [(1, 0, 900), (2, 4, 7), (3, 8, 7)] {
            let query = catalog.query("EM001", instance, |map| {
                assert_eq!(map.source_ordinal(), expected_ordinal);
                assert_eq!(map.source_index(), expected_index);
                Some(count(17))
            });
            assert_eq!(query, NpcPopulationQuery::KnownCount(count(17)));
        }
    }

    #[test]
    fn negative_zero_and_one_all_select_first_successful_instance() {
        let catalog = catalog();
        for instance in [i32::MIN, -19, -1, 0, 1] {
            assert_eq!(
                catalog.query("em001", instance, |map| Some(count(map.source_ordinal()))),
                NpcPopulationQuery::KnownCount(count(0))
            );
        }
    }

    #[test]
    fn absent_instance_does_not_reuse_another_or_read_authority() {
        let catalog = catalog();
        for instance in [4, i32::MAX] {
            assert_eq!(
                catalog.query("EM001", instance, |_| panic!(
                    "missing map must not be read"
                )),
                NpcPopulationQuery::MissingSourceMap
            );
        }
    }

    #[test]
    fn file_matching_does_not_create_path_whitespace_or_title_aliases() {
        let catalog = catalog();
        assert_eq!(
            catalog.query("eM001", 1, |_| Some(count(2))),
            NpcPopulationQuery::KnownCount(count(2))
        );
        for file_name in [" EM001", "EM001 ", "EM001.map", "maps/EM001", "Event Arena"] {
            assert_eq!(
                catalog.query(file_name, 1, |_| panic!("alias must not be read")),
                NpcPopulationQuery::MissingSourceMap
            );
        }
        let exact =
            NpcLoadedMapCatalog::from_successful_source_order(vec![map(0, 7, " EM001 ")]).unwrap();
        assert_eq!(
            exact.query("EM001", 1, |_| panic!("stored spelling is not trimmed")),
            NpcPopulationQuery::MissingSourceMap
        );
        assert_eq!(
            exact.query(" EM001 ", 1, |_| Some(count(1))),
            NpcPopulationQuery::KnownCount(count(1))
        );
    }

    #[test]
    fn unsupported_culture_names_never_become_fake_missing_or_known_counts() {
        for file_name in ["矿洞", "EM00İ"] {
            assert_eq!(
                NpcLoadedSourceMap::from_successful_load(0, 1, file_name.to_owned()),
                Err(NpcPopulationCatalogError::UnsupportedNonAsciiFileName)
            );
            assert_eq!(
                catalog().query(file_name, 1, |_| panic!(
                    "unsupported query must not be read"
                )),
                NpcPopulationQuery::AuthorityUnavailable
            );
        }
    }

    #[test]
    fn known_zero_missing_source_and_unknown_authority_are_distinct() {
        let catalog = catalog();
        assert_eq!(
            catalog.query("EM001", 1, |_| Some(count(0))),
            NpcPopulationQuery::KnownCount(count(0))
        );
        assert_eq!(
            catalog.query("EM001", 1, |_| None),
            NpcPopulationQuery::AuthorityUnavailable
        );
        assert_eq!(
            catalog.query("UNLOADED", 1, |_| panic!("unloaded map must not be read")),
            NpcPopulationQuery::MissingSourceMap
        );
        let empty = NpcLoadedMapCatalog::from_successful_source_order(Vec::new()).unwrap();
        assert!(empty.maps().is_empty());
        assert_eq!(
            empty.query("EM001", 1, |_| panic!("empty directory must not be read")),
            NpcPopulationQuery::MissingSourceMap
        );
    }

    #[test]
    fn population_count_cannot_wrap_the_source_i32_count_range() {
        assert_eq!(count(0).get(), 0);
        assert_eq!(count(i32::MAX as usize).get(), i32::MAX);
        assert_eq!(
            NpcPopulationCount::try_from_usize(i32::MAX as usize + 1),
            None
        );
        assert_eq!(NpcPopulationCount::try_from_usize(usize::MAX), None);
    }

    #[test]
    fn incomplete_check_is_an_omission_and_explicit_extra_tokens_are_ignored() {
        for arguments in [&[][..], &[">="][..], &[">=", "1"][..]] {
            assert_eq!(NpcCheckHum::parse_source_arguments(arguments), None);
        }
        let check = check(&["==", "9", "EM001", "2", "not-a-third-argument"]);
        assert_eq!(check.file_name(), "EM001");
        assert_eq!(
            check.evaluate(&catalog(), |map| {
                assert_eq!(map.source_ordinal(), 4);
                Some(count(9))
            }),
            Ok(true)
        );
    }

    #[test]
    fn omitted_instance_defaults_to_first_even_with_multiple_successful_maps() {
        assert_eq!(
            check(&["==", "3", "EM001"]).evaluate(&catalog(), |map| {
                assert_eq!(map.source_ordinal(), 0);
                Some(count(3))
            }),
            Ok(true)
        );
    }

    #[test]
    fn i32_parser_preserves_sign_extrema_outer_ascii_space_and_trailing_nuls() {
        for (raw, expected) in [
            ("2147483647", i32::MAX),
            ("-2147483648", i32::MIN),
            ("+0001", 1),
            ("-0", 0),
            (" \t-19\r\n", -19),
            ("1\0\0", 1),
            ("+1 \0", 1),
        ] {
            assert_eq!(try_parse_source_i32(raw), Some(expected), "{raw:?}");
        }
    }

    #[test]
    fn i32_parser_rejects_overflow_internal_space_and_non_source_numeric_forms() {
        for raw in [
            "2147483648",
            "-2147483649",
            "+2147483648",
            "",
            "+",
            "--1",
            "- 1",
            "1 0",
            "1_0",
            "1.0",
            "1e1",
            "0x10",
            "１",
            "−1",
            "\u{00a0}1\u{00a0}",
            "1\0 ",
            "\01",
        ] {
            assert_eq!(try_parse_source_i32(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn invalid_count_short_circuits_instance_map_authority_and_operator() {
        for raw_count in ["wrong", "2147483648", "1 0"] {
            assert_eq!(
                check(&["invalid", raw_count, "矿洞", "bad-instance"])
                    .evaluate(&catalog(), |_| panic!("invalid count must not query")),
                Ok(false)
            );
        }
    }

    #[test]
    fn invalid_instance_short_circuits_map_authority_and_operator() {
        for instance in ["", "2147483648", "-2147483649", "1_0", "1 2"] {
            assert_eq!(
                check(&["invalid", "1", "EM001", instance])
                    .evaluate(&catalog(), |_| panic!("invalid instance must not query")),
                Ok(false)
            );
        }
    }

    #[test]
    fn substituted_numeric_argument_is_not_retokenized() {
        assert_eq!(
            check(&[">=", "1 2", "EM001"]).evaluate(&catalog(), |_| panic!(
                "one substituted value is not two tokens"
            )),
            Ok(false)
        );
    }

    #[test]
    fn missing_source_map_returns_false_before_invalid_operator_is_checked() {
        for arguments in [
            ["invalid", "1", "ABSENT", "1"],
            ["invalid", "1", "EM001", "4"],
        ] {
            assert_eq!(
                check(&arguments).evaluate(&catalog(), |_| panic!("missing map must not query")),
                Ok(false)
            );
        }
    }

    #[test]
    fn unavailable_authority_never_becomes_false_or_zero_even_for_invalid_operator() {
        for operator in ["==", "invalid"] {
            assert_eq!(
                check(&[operator, "0", "EM001"]).evaluate(&catalog(), |_| None),
                Err(NpcPopulationError::AuthorityUnavailable)
            );
        }
    }

    #[test]
    fn invalid_operator_throws_only_after_count_for_a_real_map_is_read() {
        let reads = Cell::new(0);
        for operator in ["=", "<>", "== ", " >=", "eq", ""] {
            assert_eq!(
                check(&[operator, "2", "EM001"]).evaluate(&catalog(), |_| {
                    reads.set(reads.get() + 1);
                    Some(count(2))
                }),
                Err(NpcPopulationError::InvalidComparisonOperator)
            );
        }
        assert_eq!(reads.get(), 6);
    }

    #[test]
    fn all_six_source_comparisons_use_real_nonnegative_population() {
        for (operator, expected_count, expected) in [
            ("<", "3", true),
            ("<", "2", false),
            (">", "1", true),
            (">", "2", false),
            ("<=", "2", true),
            ("<=", "1", false),
            (">=", "2", true),
            (">=", "3", false),
            ("==", "2", true),
            ("==", "3", false),
            ("!=", "3", true),
            ("!=", "2", false),
            (">", "-2147483648", true),
            ("<", "2147483647", true),
        ] {
            assert_eq!(
                check(&[operator, expected_count, "EM001"])
                    .evaluate(&catalog(), |_| Some(count(2))),
                Ok(expected),
                "{operator} {expected_count}"
            );
        }
    }
}
