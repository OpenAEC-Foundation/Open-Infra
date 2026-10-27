#!/usr/bin/env python3
"""Make small test files in each format the GIS file import reads, around
Zürich Hauptbahnhof (LV95 2683250 1248000, 8.5402 E 47.3779 N).

Run: python3 tests/data/gisbestanden/maak.py
"""

import json
import sqlite3
import struct
from pathlib import Path

HERE = Path(__file__).resolve().parent
E0, N0 = 2683250.0, 1248000.0
LON0, LAT0 = 8.5402, 47.3779


def geojson():
    d = 0.0008
    fc = {
        "type": "FeatureCollection",
        "features": [
            {"type": "Feature", "properties": {"name": "Block A", "kind": "building", "floors": 5},
             "geometry": {"type": "Polygon", "coordinates": [[[LON0, LAT0], [LON0 + d, LAT0], [LON0 + d, LAT0 + d], [LON0, LAT0 + d], [LON0, LAT0]]]}},
            {"type": "Feature", "properties": {"name": "Bahnhofstrasse", "kind": "road"},
             "geometry": {"type": "LineString", "coordinates": [[LON0 - d, LAT0 - d], [LON0 + 2 * d, LAT0 + 0.5 * d]]}},
            {"type": "Feature", "properties": {"name": "Tree 1", "kind": "tree"},
             "geometry": {"type": "Point", "coordinates": [LON0 + 0.5 * d, LAT0 - 0.5 * d]}},
        ],
    }
    (HERE / "zurich.geojson").write_text(json.dumps(fc, indent=1))


def csv():
    rows = ["id;x;y;type", "1;2683200,5;1247990;manhole", "2;2683260;1248030;manhole", "3;2683300;1248060;hydrant"]
    (HERE / "zurich_lv95.csv").write_text("\n".join(rows) + "\n")


def gpx():
    pts = "".join(f'<trkpt lat="{LAT0 + i * 0.0002}" lon="{LON0 + i * 0.0003}"/>' for i in range(6))
    (HERE / "zurich.gpx").write_text(
        f'<?xml version="1.0"?><gpx version="1.1" creator="open-infra-test"><wpt lat="{LAT0}" lon="{LON0}"><name>Start</name></wpt><trk><name>Walk</name><trkseg>{pts}</trkseg></trk></gpx>'
    )


def kml():
    (HERE / "zurich.kml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>'
        f'<Placemark><name>Platform</name><ExtendedData><Data name="track"><value>3</value></Data></ExtendedData><LineString><coordinates>{LON0},{LAT0},0 {LON0 + 0.001},{LAT0 + 0.0003},0</coordinates></LineString></Placemark>'
        "</Document></kml>"
    )


def shapefile():
    """A PolyLine shapefile in LV95 with two records and an ESRI .prj."""
    parts = [[(E0, N0), (E0 + 60, N0 + 20)], [(E0 + 10, N0 - 30), (E0 + 40, N0 - 10), (E0 + 70, N0 - 25)]]
    recs = []
    for pts in parts:
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        body = struct.pack("<i4d2i", 3, min(xs), min(ys), max(xs), max(ys), 1, len(pts)) + struct.pack("<i", 0)
        body += b"".join(struct.pack("<2d", *p) for p in pts)
        recs.append(body)
    allx = [p[0] for ps in parts for p in ps]
    ally = [p[1] for ps in parts for p in ps]

    def header(length_words):
        return struct.pack(">7i", 9994, 0, 0, 0, 0, 0, length_words) + struct.pack("<2i4d4d", 1000, 3, min(allx), min(ally), max(allx), max(ally), 0, 0, 0, 0)

    shp = b""
    shx_recs = b""
    offset = 50
    for i, body in enumerate(recs):
        shp += struct.pack(">2i", i + 1, len(body) // 2) + body
        shx_recs += struct.pack(">2i", offset, len(body) // 2)
        offset += 4 + len(body) // 2
    (HERE / "zurich_lines.shp").write_bytes(header(50 + len(shp) // 2) + shp)
    (HERE / "zurich_lines.shx").write_bytes(header(50 + len(shx_recs) // 2) + shx_recs)
    # dBase III: one character field NAME (20) and one numeric WIDTH (5.1).
    fields = [(b"NAME", b"C", 20, 0), (b"WIDTH", b"N", 5, 1)]
    rec_len = 1 + sum(f[2] for f in fields)
    hdr_len = 32 + 32 * len(fields) + 1
    dbf = struct.pack("<4BIHH20x", 3, 126, 10, 27, len(parts), hdr_len, rec_len)
    for name, typ, size, dec in fields:
        dbf += name.ljust(11, b"\0") + typ + b"\0" * 4 + bytes([size, dec]) + b"\0" * 14
    dbf += b"\r"
    for name, width in [("Track west", 3.5), ("Track east", 4.0)]:
        dbf += b" " + name.encode().ljust(20) + f"{width:5.1f}".encode()
    dbf += b"\x1a"
    (HERE / "zurich_lines.dbf").write_bytes(dbf)
    (HERE / "zurich_lines.prj").write_text('PROJCS["CH1903+_LV95",GEOGCS["GCS_CH1903+",DATUM["D_CH1903+",SPHEROID["Bessel_1841",6377397.155,299.1528128]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Hotine_Oblique_Mercator_Azimuth_Center"],UNIT["Meter",1.0]]')


def geopackage():
    path = HERE / "zurich.gpkg"
    path.unlink(missing_ok=True)
    db = sqlite3.connect(path)
    db.executescript(
        """
        CREATE TABLE gpkg_spatial_ref_sys (srs_name TEXT, srs_id INTEGER PRIMARY KEY, organization TEXT, organization_coordsys_id INTEGER, definition TEXT, description TEXT);
        CREATE TABLE gpkg_contents (table_name TEXT PRIMARY KEY, data_type TEXT, identifier TEXT, description TEXT, last_change TEXT, min_x REAL, min_y REAL, max_x REAL, max_y REAL, srs_id INTEGER);
        CREATE TABLE gpkg_geometry_columns (table_name TEXT, column_name TEXT, geometry_type_name TEXT, srs_id INTEGER, z INTEGER, m INTEGER);
        CREATE TABLE parcels (fid INTEGER PRIMARY KEY, geom BLOB, number TEXT, area REAL);
        INSERT INTO gpkg_spatial_ref_sys VALUES ('CH1903+ / LV95', 2056, 'EPSG', 2056, 'undefined', '');
        INSERT INTO gpkg_contents VALUES ('parcels', 'features', 'parcels', '', '2026-10-27', 0, 0, 0, 0, 2056);
        INSERT INTO gpkg_geometry_columns VALUES ('parcels', 'geom', 'POLYGON', 2056, 0, 0);
        """
    )
    for i, (x, y) in enumerate([(E0 - 80, N0 + 40), (E0 - 40, N0 + 40)]):
        ring = [(x, y), (x + 35, y), (x + 35, y + 30), (x, y + 30), (x, y)]
        wkb = struct.pack("<BII", 1, 3, 1) + struct.pack("<I", len(ring)) + b"".join(struct.pack("<2d", *p) for p in ring)
        blob = b"GP" + bytes([0, 0b00000001]) + struct.pack("<i", 2056) + wkb
        db.execute("INSERT INTO parcels (geom, number, area) VALUES (?, ?, ?)", (blob, f"P-{i + 1}", 1050.0))
    db.commit()
    db.close()


for fn in (geojson, csv, gpx, kml, shapefile, geopackage):
    fn()
print("written to", HERE)
