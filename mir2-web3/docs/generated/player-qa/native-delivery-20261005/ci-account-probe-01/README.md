# Fixed account-token authority probe

The actual first CI run37240009948 used the user-token verification endpoint and returned401 before any bucket call. Its safe receipt and actual job metadata are retained; both old Web jobs were skipped. This denial alone does not establish token expiration, and no write/deploy/route permission was tested.

The account-token variant changes only the docstring and fixed official account verification path. Worker32-case RED retains31 old passes and one new literal-path failure; GREEN32/32 and independent root32/32 pass with overlap. All fixture inputs are fake. Link topology stays inert JSON metadata. Production transport remains verified TLS, no redirects/proxies, at most two fixed GETs, normalized non-secret receipt and fail-closed write/close handling. Actual account CI is a subsequent gate; offline tests prove no live authority.
