//! High-level builder API for querying `rust-analyzer`.
//!
//! [`MethodQuery`] provides a chainable interface over [`query_methods`] and
//! [`query_definition`], keeping call sites readable when multiple options are
//! involved. [`filter_methods`] is also available as a standalone function for
//! callers who already have a [`Vec<Method>`] and just need filtering.
//!
//! # Examples
//!
//! ```no_run
//! use rust_meth_lib::query::MethodQuery;
//! use rust_meth_lib::analyzer::find_rust_analyzer;
//!
//! let ra_path = find_rust_analyzer().unwrap();
//!
//! // Simple query with filter
//! let methods = MethodQuery::new("Vec<u8>")
//!     .filter("drain")
//!     .run(&ra_path)
//!     .unwrap();
//!
//! // Query with definitions, third-party type
//! let results = MethodQuery::new("serde_json::Value")
//!     .deps(r#"serde_json = "1.0""#)
//!     .filter("as_")
//!     .run_with_definitions(&ra_path)
//!     .unwrap();
//!
//! for r in results {
//!     if let Some(def) = r.definition {
//!         println!("{} → {}:{}", r.method.name, def.path, def.line + 1);
//!     }
//! }
//! ```

use crate::analyzer::{Definition, Method, query_definition, query_methods};
use crate::error::Result;

// -- filter_methods --

/// Filters and ranks a slice of methods against a query string.
///
/// Scoring tiers (highest first):
/// - **Exact match**: `name == query`
/// - **Prefix match**: `name.starts_with(query)`
/// - **Substring match**: `name.contains(query)`
///
/// Methods that don't match at all are excluded. Results are stable within
/// each tier (input order is preserved for equal scores).
///
/// Pass `query = ""` or use [`filter_methods`] with an empty string to return
/// all methods in input order.
///
/// # Example
///
/// ```no_run
/// use rust_meth_lib::query::filter_methods;
/// use rust_meth_lib::analyzer::{Method, find_rust_analyzer, query_methods};
///
/// let ra_path = find_rust_analyzer().unwrap();
/// let methods = query_methods("HashMap<String, u32>", &ra_path, None).unwrap();
/// let filtered = filter_methods(&methods, "get");
/// for m in filtered {
///     println!("{}", m.name);
/// }
/// ```
#[must_use]
pub fn filter_methods<'a>(methods: &'a [Method], query: &str) -> Vec<&'a Method> {
    if query.is_empty() {
        return methods.iter().collect();
    }
    let mut scored: Vec<(u8, &Method)> = methods
        .iter()
        .filter_map(|m| {
            let score = if m.name == query {
                3
            } else if m.name.starts_with(query) {
                2
            } else if m.name.contains(query) {
                1
            } else {
                return None;
            };
            Some((score, m))
        })
        .collect();

    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
    scored.into_iter().map(|(_, m)| m).collect()
}

// ── MethodResult ──────────────────────────────────────────────────────────────

/// A method paired with its optional source location.
///
/// Returned by [`MethodQuery::run_with_definitions`] and
/// [`query_definition_for_methods`]. The `definition` field is `None` when
/// `rust-analyzer` has no source location for the method (e.g. compiler
/// built-ins) the method is still included in results.
pub struct MethodResult {
    /// The method returned by the completion query.
    pub method: Method,
    /// The source location of the method, if resolvable.
    pub definition: Option<Definition>,
}

// ── query_definition_for_methods ─────────────────────────────────────────────

/// Resolves source locations for a slice of methods in parallel.
///
/// Runs one [`query_definition`] call per method using [`std::thread::scope`].
/// Methods where `rust-analyzer` returns no location are included with
/// `definition: None` rather than being silently dropped.
///
/// # Errors
///
/// Returns an error if any underlying LSP session fails fatally. Per-method
/// `Ok(None)` results (no source location found) are not errors.
///
/// # Example
///
/// ```no_run
/// use rust_meth_lib::query::query_definition_for_methods;
/// use rust_meth_lib::analyzer::{find_rust_analyzer, query_methods};
///
/// let ra_path = find_rust_analyzer().unwrap();
/// let methods = query_methods("Vec<u8>", &ra_path, None).unwrap();
/// let results = query_definition_for_methods(&methods, "Vec<u8>", None, &ra_path).unwrap();
///
/// for r in results {
///     match r.definition {
///         Some(def) => println!("{} → {}:{}", r.method.name, def.path, def.line + 1),
///         None => println!("{} → (no source location)", r.method.name),
///     }
/// }
/// ```
/// ## Panics
///
/// This function may panic if any spawned background thread computing the method
/// definitions panics or is aborted
pub fn query_definition_for_methods(
    methods: &[Method],
    type_name: &str,
    deps: Option<&str>,
    ra_path: &std::path::Path,
) -> Result<Vec<MethodResult>> {
    // query_definition returns Result<Option<Definition>>, a fatal LSP error
    // is Err, "no location found" is Ok(None). We propagate fatal errors and
    // treat Ok(None) as a valid result.
    //
    // Collect errors separately so one fatal failure doesn't silently drop
    // the rest, we return the first error if any occurred.
    let results: Vec<Result<MethodResult>> = std::thread::scope(|s| {
        methods
            .iter()
            .map(|method| {
                s.spawn(move || {
                    let definition = query_definition(type_name, &method.name, ra_path, deps)?;
                    Ok(MethodResult {
                        method: Method {
                            name: method.name.clone(),
                            detail: method.detail.clone(),
                            documentation: method.documentation.clone(),
                        },
                        definition,
                    })
                })
            })
            .map(|h| h.join().expect("definition query thread should not panic"))
            .collect()
    });

    results.into_iter().collect()
}

// -- MethodQuery --

/// A chainable builder for method queries.
///
/// Construct with [`MethodQuery::new`], optionally set [`deps`](MethodQuery::deps)
/// and [`filter`](MethodQuery::filter), then call [`run`](MethodQuery::run) or
/// [`run_with_definitions`](MethodQuery::run_with_definitions) to execute.
///
/// # Example
///
/// ```no_run
/// use rust_meth_lib::query::MethodQuery;
/// use rust_meth_lib::analyzer::find_rust_analyzer;
///
/// let ra_path = find_rust_analyzer().unwrap();
///
/// let methods = MethodQuery::new("String")
///     .filter("push")
///     .run(&ra_path)
///     .unwrap();
///
/// for m in methods {
///     println!("{}", m.name);
/// }
/// ```
pub struct MethodQuery<'a> {
    type_name: &'a str,
    deps: Option<&'a str>,
    filter: Option<&'a str>,
}

impl<'a> MethodQuery<'a> {
    /// Creates a new query for the given Rust type expression.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_meth_lib::query::MethodQuery;
    /// let q = MethodQuery::new("Vec<u8>");
    /// ```
    #[must_use]
    pub const fn new(type_name: &'a str) -> Self {
        Self {
            type_name,
            deps: None,
            filter: None,
        }
    }

    /// Sets the TOML dependency string for third-party crate types.
    ///
    /// Accepts the same format as `query_methods`, a raw TOML snippet that
    /// would appear under `[dependencies]`. Multiple crates are
    /// newline-separated.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_meth_lib::query::MethodQuery;
    /// use rust_meth_lib::analyzer::find_rust_analyzer;
    ///
    /// let ra_path = find_rust_analyzer().unwrap();
    /// let methods = MethodQuery::new("serde_json::Value")
    ///     .deps(r#"serde_json = "1.0""#)
    ///     .run(&ra_path)
    ///     .unwrap();
    /// ```
    #[must_use]
    pub const fn deps(mut self, deps: &'a str) -> Self {
        self.deps = Some(deps);
        self
    }

    /// Applies a filter to the results after the query completes.
    ///
    /// Uses the same scoring as [`filter_methods`]: exact > prefix > substring.
    /// Methods that don't match are excluded from results.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rust_meth_lib::query::MethodQuery;
    /// use rust_meth_lib::analyzer::find_rust_analyzer;
    ///
    /// let ra_path = find_rust_analyzer().unwrap();
    /// let methods = MethodQuery::new("HashMap<String, u32>")
    ///     .filter("get")
    ///     .run(&ra_path)
    ///     .unwrap();
    /// ```
    #[must_use]
    pub const fn filter(mut self, query: &'a str) -> Self {
        self.filter = Some(query);
        self
    }

    /// Executes the query and returns matching methods.
    ///
    /// Runs [`query_methods`] then applies the filter if one was set.
    /// Returns owned [`Method`] values.
    ///
    /// # Errors
    ///
    /// Propagates any error from the underlying LSP session.
    pub fn run(self, ra_path: &std::path::Path) -> Result<Vec<Method>> {
        let methods = query_methods(self.type_name, ra_path, self.deps)?;
        let filtered = match self.filter {
            Some(q) => filter_methods(&methods, q).into_iter().cloned().collect(),
            None => methods,
        };
        Ok(filtered)
    }

    /// Executes the query, applies the filter, and resolves source locations
    /// for each matching method in parallel.
    ///
    /// Methods where `rust-analyzer` has no source location are included with
    /// `definition: None` rather than being silently dropped.
    ///
    /// # Errors
    ///
    /// Propagates any fatal LSP error. Per-method `Ok(None)` definition
    /// results are not errors.
    pub fn run_with_definitions(self, ra_path: &std::path::Path) -> Result<Vec<MethodResult>> {
        let type_name = self.type_name;
        let deps = self.deps;
        let methods = self.run(ra_path)?;
        query_definition_for_methods(&methods, type_name, deps, ra_path)
    }
}

// -- Method: Clone --
//
// `run` needs to clone filtered methods out of the temporary Vec<&Method>.
// If Method doesn't already derive Clone, add it to parse.rs.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Method;

    fn make_methods(names: &[&str]) -> Vec<Method> {
        names
            .iter()
            .map(|&name| Method {
                name: name.to_string(),
                detail: None,
                documentation: None,
            })
            .collect()
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let methods = make_methods(&["push", "pop", "len"]);
        let result = filter_methods(&methods, "");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn filter_exact_match_scores_highest() {
        let methods = make_methods(&["get_mut", "get", "get_key_value"]);
        let result = filter_methods(&methods, "get");
        assert_eq!(result[0].name, "get", "exact match should be first");
    }

    #[test]
    fn filter_prefix_before_substring() {
        let methods = make_methods(&["forget", "get_mut", "get"]);
        let result = filter_methods(&methods, "get");
        let names: Vec<&str> = result.iter().map(|m| m.name.as_str()).collect();
        // "get" exact first, then prefix "get_mut", then substring "forget"
        assert_eq!(names, ["get", "get_mut", "forget"]);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let methods = make_methods(&["push", "pop", "len"]);
        let result = filter_methods(&methods, "zzz");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_excludes_non_matching() {
        let methods = make_methods(&["push", "pop", "get", "len"]);
        let result = filter_methods(&methods, "get");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "get");
    }
}
