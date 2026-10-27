#!/usr/bin/env python3
"""Turn the checked web service list of GIS2BIM into Open Infra presets.

Source: GIS2BIM_Data.json (branch `worldwide-servers` of
OpenAEC-Foundation/GIS2BIM). Every service in it was checked with
GIS2BIM's supportfiles/check_servers.py: GetCapabilities answered and a sample
request returned data. Only public services without an API key are listed.

WMS, WFS and XYZ tile templates are kept (the service kinds Open Infra reads);
WCS, ArcGIS REST, downloads and WFS with curves are left out. Dutch and Flemish
services that Open Infra already has as built-in presets are skipped.

Usage:
  git -C ../GIS2BIM show worldwide-servers:GIS2BIM_Data.json > /tmp/gis2bim.json
  python3 tools/import_gis2bim_servers.py /tmp/gis2bim.json > assets/servers/world.json
"""

import json
import re
import sys
from urllib.parse import parse_qs, urlsplit

# Country / region code → (Dutch, English) name.
NAMES = {
    "AR": ("Argentinië", "Argentina"), "AT": ("Oostenrijk", "Austria"), "AU": ("Australië", "Australia"),
    "BE": ("België", "Belgium"), "BG": ("Bulgarije", "Bulgaria"), "BR": ("Brazilië", "Brazil"),
    "CA": ("Canada", "Canada"), "CH": ("Zwitserland", "Switzerland"), "CL": ("Chili", "Chile"),
    "CO": ("Colombia", "Colombia"), "CZ": ("Tsjechië", "Czechia"), "DE": ("Duitsland", "Germany"),
    "DK": ("Denemarken", "Denmark"), "EE": ("Estland", "Estonia"), "ES": ("Spanje", "Spain"),
    "FI": ("Finland", "Finland"), "FR": ("Frankrijk", "France"), "GB": ("Verenigd Koninkrijk", "United Kingdom"),
    "GR": ("Griekenland", "Greece"), "HK": ("Hongkong", "Hong Kong"), "HR": ("Kroatië", "Croatia"),
    "ID": ("Indonesië", "Indonesia"), "IE": ("Ierland", "Ireland"), "IL": ("Israël", "Israel"),
    "IN": ("India", "India"), "IS": ("IJsland", "Iceland"), "IT": ("Italië", "Italy"), "JP": ("Japan", "Japan"),
    "LI": ("Liechtenstein", "Liechtenstein"), "LT": ("Litouwen", "Lithuania"), "LU": ("Luxemburg", "Luxembourg"),
    "LV": ("Letland", "Latvia"), "MX": ("Mexico", "Mexico"), "NL": ("Nederland", "Netherlands"),
    "NO": ("Noorwegen", "Norway"), "NZ": ("Nieuw-Zeeland", "New Zealand"), "PE": ("Peru", "Peru"),
    "PH": ("Filipijnen", "Philippines"), "PL": ("Polen", "Poland"), "PT": ("Portugal", "Portugal"),
    "SE": ("Zweden", "Sweden"), "SG": ("Singapore", "Singapore"), "SI": ("Slovenië", "Slovenia"),
    "SK": ("Slowakije", "Slovakia"), "TH": ("Thailand", "Thailand"), "TW": ("Taiwan", "Taiwan"),
    "US": ("Verenigde Staten", "United States"), "UY": ("Uruguay", "Uruguay"), "ZA": ("Zuid-Afrika", "South Africa"),
    "AFRICA": ("Afrika", "Africa"), "EU": ("Europa", "Europe"), "GLOBAL": ("Wereld", "World"),
}

# Built-in presets already cover these endpoints.
SKIP_HOSTS = ("service.pdok.nl", "geo.api.vlaanderen.be/OMWRGBMRVL", "geo.api.vlaanderen.be/GRB/wfs", "tile.openstreetmap.org", "ows.terrestris.de/osm", "wms.gebco.net", "gibs.earthdata.nasa.gov/wms/epsg4326")


def param(qs, *names):
    for n in names:
        for k, v in qs.items():
            if k.lower() == n and v:
                return v[0]
    return ""


def main(path):
    data = json.load(open(path))["GIS2BIMserversRequests"]["webserverRequests"]
    out, seen = [], set()
    for e in data:
        country = e.get("country")
        if not country or country not in NAMES:
            continue
        kind = {"WMS": "wms", "WFS": "wfs", "WMTS_TMS": "xyz"}.get(e.get("service", ""))
        if not kind:
            continue
        url = e["serverrequestprefix"]
        if any(h in url for h in SKIP_HOSTS):
            continue
        layer = ""
        if kind == "xyz":
            if not all(t in url for t in ("{z}", "{x}", "{y}")):
                continue
            base = url
        else:
            parts = urlsplit(url)
            qs = parse_qs(parts.query, keep_blank_values=True)
            layer = param(qs, "layers") if kind == "wms" else param(qs, "typename", "typenames")
            # Keep vendor parameters (map=...) that select the service.
            keep = {k: v[0] for k, v in qs.items() if k.lower() in ("map",)}
            base = f"{parts.scheme}://{parts.netloc}{parts.path}"
            if keep:
                base += "?" + "&".join(f"{k}={v}" for k, v in keep.items())
        key = (kind, base, layer)
        if key in seen:
            continue
        seen.add(key)
        title = e.get("title", "")
        ident = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")[:80]
        nl, en = NAMES[country]
        lic = e.get("license", "")
        provider = e.get("provider", "")
        attribution = provider + (f" ({lic})" if lic and lic.lower() not in ("see terms",) else "")
        name = e.get("description") or title
        coverage = e.get("coverage", "")
        if coverage and coverage not in (en,) and coverage.lower() not in name.lower():
            name = f"{name} — {coverage}"
        out.append({
            "id": "w-" + ident,
            "kind": kind,
            "name": name[:140],
            "region_nl": nl,
            "region_en": en,
            "country": country,
            "category": e.get("category", ""),
            "url": base,
            "default_layer": layer,
            "attribution": attribution[:200],
            "terms": e.get("terms", ""),
            "verified": e.get("verified", ""),
        })
    order = {c: i for i, c in enumerate(["GLOBAL", "EU"])}
    out.sort(key=lambda s: (order.get(s["country"], 2), s["region_en"], s["category"], s["name"]))
    json.dump(out, sys.stdout, ensure_ascii=False, indent=0)
    print(f"{len(out)} presets", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1])
