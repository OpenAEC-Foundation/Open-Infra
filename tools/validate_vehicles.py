#!/usr/bin/env python3
"""Validate vehicles.json: schema, geometry and source cross-checks."""
import json, re, sys, collections

P = str(__import__("pathlib").Path(__file__).resolve().parents[1] / "assets" / "library" / "vehicles.json")
V = json.load(open(P))
ft = lambda v: v * 0.3048

CATS = set("car van pickup truck tractor_semitrailer truck_trailer bus coach articulated_bus emergency municipal "
           "construction agricultural two_wheeler rail special".split())
REGIONS = set("US EU DE NL UK AU GLOBAL".split())
STANDARDS = {"AASHTO 2018 Green Book", "FGSV RASt 06 / Bemessungsfahrzeuge", "Austroads", "EU 96/53/EC (max)", "UK",
             "CROW", "manufacturer", "generic"}
KINDS = set("body cab_over conventional_cab tractor semitrailer trailer dolly bus bus_rear car bike rail_car".split())
TOL = 0.025

# expected (unit index, rear overhang in m) taken directly from the sources
REAR = {
    "aashto_p": (0, ft(5.0)), "aashto_su30": (0, ft(6.0)), "aashto_city_bus": (0, ft(8.0)),
    "aashto_sbus36": (0, ft(12.0)), "aashto_sbus40": (0, ft(13.0)), "aashto_mh": (0, ft(6.0)),
    "aashto_bus45": (0, ft(9.0)), "aashto_abus": (1, ft(10.0)), "aashto_wb62": (1, ft(4.5)),
    "aashto_wb67": (1, ft(4.5)), "aashto_wb40": (1, ft(4.5)), "aashto_pt": (1, ft(12.0)),
    "aashto_pb": (1, ft(8.0)), "aashto_mhb": (1, ft(8.0)), "aashto_wb109d": (3, ft(4.5)),
    "aashto_wb67d": (3, ft(3.0)), "aashto_wb92d": (3, ft(3.0)), "aashto_wb100t": (5, ft(3.0)),
    "fgsv_pkw_2001": (0, 1.10), "fgsv_pkw_2010_proposal": (0, 1.09), "rbsv_pkw_2020": (0, 1.10),
    "fgsv_sattelzug_2001": (1, 4.25), "eu_tractor_semitrailer_16_5": (1, 4.43), "eu_truck_trailer_18_75": (1, 1.20),
    "eu_articulated_bus_18_75": (1, 3.21), "eu_bus_2axle_12": (0, 3.43), "austroads_car_b99": (0, 1.20),
    "car_renault_clio": (0, 0.667), "car_toyota_corolla": (0, 0.995), "car_suv_toyota_rav4": (0, 1.000),
    "car_suv_tesla_model_y": (0, 1.011), "car_suv_volvo_xc90": (0, 1.079), "car_mpv_nissan_e_nv200": (0, 0.850),
    "emergency_police_ford_explorer": (0, 1.169), "pickup_ford_f150": (0, 1.232),
    "van_vw_transporter_t6_swb": (0, 0.996), "van_vw_transporter_t61_lwb": (0, 0.998),
    "van_mercedes_vito_long": (0, 1.045), "van_mercedes_esprinter_standard": (0, 1.152),
    "van_renault_master_l3h2": (0, 1.024), "van_iveco_daily_l4h3": (0, 2.126),
}
# expected overall assembled length (m) from the sources
OVERALL = {
    "aashto_abus": ft(60.0), "aashto_wb40": ft(45.5), "aashto_wb62": ft(69.0), "aashto_wb67": ft(73.5),
    "aashto_wb67d": ft(72.3), "aashto_wb92d": ft(97.3), "aashto_wb100t": ft(104.8), "aashto_wb109d": ft(114.0),
    "aashto_pt": ft(48.7), "aashto_pb": ft(42.0), "aashto_mhb": ft(53.0), "fgsv_sattelzug_2001": 16.50,
    "eu_tractor_semitrailer_16_5": 16.50, "eu_truck_trailer_18_75": 18.75, "eu_articulated_bus_18_75": 18.75,
    "uk_longer_semitrailer_18_55": 18.55, "de_stadtbahnwagen_b": 28.0, "uk_cycle_with_trailer": 2.70,
}

errors, warnings = [], []
ids = collections.Counter(v["id"] for v in V)
for i, n in ids.items():
    if n > 1:
        errors.append(f"duplicate id {i}")

for v in V:
    vid = v["id"]
    if not re.fullmatch(r"[a-z0-9_]+", vid):
        errors.append(f"{vid}: bad id")
    for k in ["name_en", "name_nl", "category", "standard", "region", "source_url", "height", "turning_radius_outer",
              "units", "estimated", "notes"]:
        if k not in v:
            errors.append(f"{vid}: missing {k}")
    if v["category"] not in CATS: errors.append(f"{vid}: category {v['category']}")
    if v["region"] not in REGIONS: errors.append(f"{vid}: region {v['region']}")
    if v["standard"] not in STANDARDS: errors.append(f"{vid}: standard {v['standard']}")
    if not str(v["source_url"]).startswith("http"): errors.append(f"{vid}: source_url")
    if not isinstance(v["estimated"], list): errors.append(f"{vid}: estimated not list")
    if not v["units"]: errors.append(f"{vid}: no units")
    for ui, u in enumerate(v["units"]):
        tag = f"{vid}[{ui}]"
        if u["kind"] not in KINDS: errors.append(f"{tag}: kind {u['kind']}")
        if not (u["length"] > 0 and u["width"] > 0): errors.append(f"{tag}: length/width")
        xs = [a["x"] for a in u["axles"]]
        if not xs: errors.append(f"{tag}: no axles"); continue
        if xs != sorted(xs): errors.append(f"{tag}: axles not sorted")
        if any(x < 0 or x >= u["length"] for x in xs): errors.append(f"{tag}: axle x outside [0, length): {xs}")
        if abs(u["front_overhang"] - xs[0]) > 1e-9: errors.append(f"{tag}: front_overhang != first axle x")
        wbs = [round(b - a, 3) for a, b in zip(xs, xs[1:])]
        rear = round(u["length"] - xs[-1], 3)
        total = round(u["front_overhang"] + sum(wbs) + rear, 3)
        if abs(total - u["length"]) > 1e-6: errors.append(f"{tag}: FO+WB+RO {total} != L {u['length']}")
        for a in u["axles"]:
            if a["track"] < 0 or a["track"] > u["width"]: errors.append(f"{tag}: track {a['track']} > width")
            if not isinstance(a["dual"], bool) or not isinstance(a["steer"], bool): errors.append(f"{tag}: flags")
        if u["hitch_x"] is not None and not (0 <= u["hitch_x"] <= u["length"]):
            errors.append(f"{tag}: hitch_x outside unit")
        if ui > 0 and u["kingpin_x"] is None: errors.append(f"{tag}: towed unit without kingpin_x")
        if ui < len(v["units"]) - 1 and u["hitch_x"] is None: errors.append(f"{tag}: towing unit without hitch_x")
    # source rear overhang check
    if vid in REAR:
        ui, ro = REAR[vid]
        u = v["units"][ui]
        xs = [a["x"] for a in u["axles"]]
        grp = [xs[-1]]
        for x in reversed(xs[:-1]):
            if grp[-1] - x <= 1.5: grp.append(x)
            else: break
        ref = xs[-1] if vid == "aashto_bus45" else sum(grp) / len(grp)  # BUS-45: RO from back axle (footnote a)
        got = u["length"] - ref
        if abs(got - ro) > TOL:
            warnings.append(f"{vid}: rear overhang {got:.3f} vs source {ro:.3f}")
    # assembled overall length
    pos, end = 0.0, 0.0
    for ui, u in enumerate(v["units"]):
        if ui > 0:
            prev = v["units"][ui - 1]
            pos = prev_pos + prev["hitch_x"] - u["kingpin_x"]
        end = max(end, pos + u["length"])
        prev_pos = pos
    v["_overall"] = end
    if vid in OVERALL and abs(end - OVERALL[vid]) > TOL:
        warnings.append(f"{vid}: assembled length {end:.2f} vs source {OVERALL[vid]:.2f} (diff {end-OVERALL[vid]:+.2f})")

print(f"{len(V)} vehicles checked")
print("ERRORS:", len(errors)); [print("  ", e) for e in errors]
print("WARNINGS:", len(warnings)); [print("  ", w) for w in warnings]
print("\nPer category:", dict(collections.Counter(v["category"] for v in V)))
print("Per region:", dict(collections.Counter(v["region"] for v in V)))
print("Per standard:", dict(collections.Counter(v["standard"] for v in V)))
est = collections.Counter(e for v in V for e in v["estimated"])
print("Estimated fields:", dict(est))
print("Vehicles without any estimate:", [v["id"] for v in V if not v["estimated"]])
sys.exit(1 if errors else 0)
