package com.mir2.web3;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.regex.Pattern;
import org.json.JSONException;
import org.json.JSONObject;
import org.json.JSONTokener;

/** Preserve Rust's u64 wire integers before Android org.json can round them. */
final class WireJson {
    private static final int MAX_BYTES = 1024 * 1024;
    private static final int MAX_DEPTH = 64;
    private static final BigInteger MAX_UNSIGNED = new BigInteger("18446744073709551615");
    private static final Pattern INTEGER = Pattern.compile("-?(0|[1-9][0-9]*)");
    private static final Pattern NUMBER = Pattern.compile(
            "-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?");

    private WireJson() {}

    static JSONObject decode(String raw) throws JSONException {
        if (raw == null || raw.length() > MAX_BYTES
                || raw.getBytes(StandardCharsets.UTF_8).length > MAX_BYTES) {
            throw new JSONException("Wire JSON exceeds byte limit");
        }
        JSONTokener tokener = new LosslessTokener(raw);
        Object value = tokener.nextValue();
        if (!(value instanceof JSONObject) || tokener.nextClean() != 0) {
            throw tokener.syntaxError("Expected one wire JSON object");
        }
        return (JSONObject) value;
    }

    /** IDs are numeric protocol integers, never strings or rounded decimals. */
    static String unsignedIdentity(Object value) {
        if (value instanceof Integer || value instanceof Long) {
            return ((Number) value).longValue() > 0 ? value.toString() : null;
        }
        if (value instanceof BigInteger) {
            BigInteger number = (BigInteger) value;
            if (number.signum() > 0 && number.compareTo(MAX_UNSIGNED) <= 0) return number.toString();
        }
        return null;
    }

    /** Standard org.json handles objects, arrays and strings; only numbers differ. */
    private static final class LosslessTokener extends JSONTokener {
        private int depth;

        LosslessTokener(String raw) { super(raw); }

        @Override public Object nextValue() throws JSONException {
            if (++depth > MAX_DEPTH) {
                depth--;
                throw syntaxError("Wire JSON exceeds nesting limit");
            }
            try {
                char first = nextClean();
                if (first == 0) throw syntaxError("Missing wire JSON value");
                back();
                if (first != '-' && (first < '0' || first > '9')) return super.nextValue();
                String token = nextTo("{}[]/\\:,=;# \t\f");
                if (INTEGER.matcher(token).matches()) {
                    // Bound allocation before creating a BigInteger from untrusted input.
                    if (token.length() > 20) throw syntaxError("Wire integer exceeds 64-bit range");
                    try {
                        long number = Long.parseLong(token);
                        if (number >= Integer.MIN_VALUE && number <= Integer.MAX_VALUE) return (int) number;
                        return number;
                    } catch (NumberFormatException overflow) {
                        if (token.charAt(0) == '-') throw syntaxError("Wire integer exceeds signed range");
                        BigInteger number = new BigInteger(token);
                        if (number.compareTo(MAX_UNSIGNED) > 0) throw syntaxError("Wire integer exceeds unsigned range");
                        return number;
                    }
                }
                if (!NUMBER.matcher(token).matches()) throw syntaxError("Invalid wire JSON number");
                // Keep ordinary decimal/exponent semantics, but do not accept them as IDs.
                return new JSONTokener(token).nextValue();
            } finally {
                depth--;
            }
        }
    }
}
