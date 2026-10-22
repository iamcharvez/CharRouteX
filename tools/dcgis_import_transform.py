"""DCGIS import-stage transformer for SOUTH COT INFRA.

Paste this entire file into the Python panel on the KMZ import page, then use
Preview transformation. It never writes during preview.

DCGIS import fields used here:
    type, code, name, status, notes, buffer_count, cores_per_buffer
"""

import re


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# DCGIS does not have a separate distribution-point type. In this network,
# map source DP assets to NAP while retaining the DP classification in notes.
# Change to False if your engineering standard requires them to stay Unknown.
MAP_DP_TO_NAP = True

# Only use layouts that your engineering team has approved. These values
# describe DCGIS buffer_count x cores_per_buffer, not just a display label.
VERIFIED_CABLE_LAYOUTS = {
    12: (1, 12),
    24: (2, 12),
    48: (4, 12),
}

# Keep imported status unless a cable needs manual capacity review.
REVIEW_STATUS = "planned"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _flatten_source(value):
    """Return searchable text from nested KML source/ExtendedData values."""
    if isinstance(value, dict):
        parts = []
        for key, item in value.items():
            parts.append(str(key))
            parts.append(_flatten_source(item))
        return " ".join(parts)
    if isinstance(value, (list, tuple)):
        return " ".join(_flatten_source(item) for item in value)
    return "" if value is None else str(value)


def _source_value(asset, key):
    """Find a case-insensitive value in nested source properties."""
    wanted = key.lower()

    def visit(value):
        if isinstance(value, dict):
            for item_key, item_value in value.items():
                if str(item_key).lower() == wanted:
                    return item_value
                found = visit(item_value)
                if found is not None:
                    return found
        elif isinstance(value, (list, tuple)):
            for item in value:
                found = visit(item)
                if found is not None:
                    return found
        return None

    return visit(asset.get("source") or {})


def _append_note(asset, message):
    """Append one idempotent migration note within DCGIS's 4,000-char limit."""
    current = str(asset.get("notes") or "").strip()
    if message.lower() in current.lower():
        return current
    result = (current + (" | " if current else "") + message).strip()
    return result[:4000]


def _capacity_hints(asset):
    """Return unique core-count hints found in ExtendedData/name/notes."""
    hints = []
    explicit = _source_value(asset, "capacity_cores")
    if explicit is not None and str(explicit).strip().isdigit():
        hints.append(int(str(explicit).strip()))

    text = " ".join([
        str(asset.get("name") or ""),
        str(asset.get("notes") or ""),
        _flatten_source(asset.get("source") or {}),
    ])
    for match in re.finditer(r"(?<!\d)(\d{1,3})\s*(?:C|CORE|CORES)\b", text, re.I):
        hints.append(int(match.group(1)))
    return sorted(set(hints))


def _point_class(asset):
    """Classify a source point by cleaned metadata, name, and notes."""
    explicit = str(_source_value(asset, "migration_type") or "").lower()
    aliases = {
        "node": "node",
        "closure": "closure",
        "nap": "nap",
        "local_convergence_point": "lcp",
        "distribution_point": "dp",
        "unclassified_point": "unknown",
    }
    if explicit in aliases:
        return aliases[explicit]

    text = " ".join([
        str(asset.get("name") or ""),
        str(asset.get("notes") or ""),
        _flatten_source(asset.get("source") or {}),
    ]).upper()

    if re.search(r"\b(NODE|POP|OLT)\b", text):
        return "node"
    if re.search(r"CLOSURE|INSERTION|INSECRTION|SPLICE|POICLOSURE", text):
        return "closure"
    if re.search(r"\bNAP\s*[-#]?\s*\d*\b|\bNP(?:\s*[-#]?\s*\d+)?\b", text):
        return "nap"
    if re.search(r"\bDP\b", text):
        return "dp"
    if re.search(r"\bLCP\b", text):
        return "lcp"
    return "unknown"


def _ensure_code(asset, patch, prefix, sequence_key):
    """Only generate a code when the importer did not supply one."""
    if not str(asset.get("code") or "").strip():
        patch["code"] = prefix + "-" + dcgis.sequence(sequence_key, width=5)


# ---------------------------------------------------------------------------
# DCGIS entry point
# ---------------------------------------------------------------------------

def transform(asset):
    geometry = str(asset.get("geometry") or "").lower()
    name = str(asset.get("name") or "").strip()

    # Fiber routes
    if geometry == "linestring":
        patch = {"type": "fiber"}
        _ensure_code(asset, patch, "FBR", "south-cot-fiber")

        hints = _capacity_hints(asset)
        if len(hints) == 1 and hints[0] in VERIFIED_CABLE_LAYOUTS:
            buffers, cores = VERIFIED_CABLE_LAYOUTS[hints[0]]
            patch["buffer_count"] = buffers
            patch["cores_per_buffer"] = cores
            patch["notes"] = _append_note(
                asset,
                "Source label indicates %s cores; imported as DCGIS logical layout %sx%s. Verify the physical buffer design against as-built records."
                % (hints[0], buffers, cores),
            )
        elif len(hints) > 1:
            # One line cannot safely represent a capacity transition.
            patch["buffer_count"] = 1
            patch["cores_per_buffer"] = 1
            patch["status"] = REVIEW_STATUS
            patch["notes"] = _append_note(
                asset,
                "CAPACITY REVIEW REQUIRED: mixed hints %s; split the route at the transition/closure."
                % ", ".join(str(value) for value in hints),
            )
        elif len(hints) == 1:
            # A core count exists but its physical buffer layout is not approved.
            patch["buffer_count"] = 1
            patch["cores_per_buffer"] = 1
            patch["status"] = REVIEW_STATUS
            patch["notes"] = _append_note(
                asset,
                "CAPACITY REVIEW REQUIRED: %s total cores found, but no verified buffer layout is configured."
                % hints[0],
            )
        else:
            # DCGIS requires positive values for fiber, so use an obvious
            # placeholder and flag it. Never treat 1x1 as verified capacity.
            patch["buffer_count"] = 1
            patch["cores_per_buffer"] = 1
            patch["status"] = REVIEW_STATUS
            patch["notes"] = _append_note(
                asset,
                "CAPACITY REVIEW REQUIRED: placeholder 1x1; verify cable schedule/as-built data.",
            )

        if not name or name.upper() == "UNTITLED PATH":
            patch["name"] = "Fiber route " + str(asset.get("code") or patch.get("code") or "unassigned")
        return patch

    # Point infrastructure
    if geometry == "point":
        source_class = _point_class(asset)
        patch = {}

        if source_class == "node":
            patch["type"] = "node"
            _ensure_code(asset, patch, "NODE", "south-cot-node")
        elif source_class == "closure":
            patch["type"] = "closure"
            _ensure_code(asset, patch, "CLS", "south-cot-closure")
        elif source_class == "nap":
            patch["type"] = "nap"
            _ensure_code(asset, patch, "NAP", "south-cot-nap")
        elif source_class == "lcp":
            patch["type"] = "lcp"
            _ensure_code(asset, patch, "LCP", "south-cot-lcp")
        elif source_class == "dp" and MAP_DP_TO_NAP:
            patch["type"] = "nap"
            _ensure_code(asset, patch, "DP", "south-cot-dp")
            patch["notes"] = _append_note(
                asset,
                "Source class: distribution point; mapped to NAP because DCGIS has no separate DP type.",
            )
        elif source_class == "dp":
            patch["notes"] = _append_note(
                asset,
                "TYPE REVIEW REQUIRED: source class is distribution point; DP-to-NAP mapping is disabled.",
            )
        else:
            patch["status"] = REVIEW_STATUS
            patch["notes"] = _append_note(
                asset,
                "TYPE REVIEW REQUIRED: point could not be classified safely.",
            )

        if not name or re.fullmatch(r"[\s,.]*", name) or name.upper() == "UNTITLED PLACEMARK":
            patch["name"] = "Unclassified point " + str(asset.get("code") or patch.get("code") or "review")
        return patch

    return {}

