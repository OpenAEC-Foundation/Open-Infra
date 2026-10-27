#!/usr/bin/env python3
"""Build vehicles.json for the Open Infra vehicle library.

All values in metres. Values taken from the cited sources; anything derived or
estimated is listed in the per-vehicle "estimated" array and explained in notes.
"""
import json

OUT = str(__import__("pathlib").Path(__file__).resolve().parents[1] / "assets" / "library" / "vehicles.json")


def ft(v):
    return round(v * 0.3048, 2)


def r2(v):
    return round(v + 1e-9, 2)


def axle(x, track, dual=False, steer=False):
    return {"x": r2(x), "track": r2(track), "dual": dual, "steer": steer}


def unit(kind, length, width, axles, hitch_x=None, kingpin_x=None):
    axles = sorted(axles, key=lambda a: a["x"])
    return {
        "kind": kind,
        "length": r2(length),
        "width": r2(width),
        "front_overhang": axles[0]["x"] if axles else None,
        "axles": axles,
        "hitch_x": None if hitch_x is None else r2(hitch_x),
        "kingpin_x": None if kingpin_x is None else r2(kingpin_x),
    }


V = []


def add(id, name_en, name_nl, category, standard, region, source_url, height,
        turning_radius_outer, units, estimated, notes):
    V.append({
        "id": id, "name_en": name_en, "name_nl": name_nl, "category": category,
        "standard": standard, "region": region, "source_url": source_url,
        "height": None if height is None else r2(height),
        "turning_radius_outer": None if turning_radius_outer is None else r2(turning_radius_outer),
        "units": units, "estimated": sorted(set(estimated)), "notes": notes.strip(),
    })


# ---------------------------------------------------------------------------
# 1. AASHTO Green Book 2018 (7th ed.) - Tables 2-4a / 2-5a (US customary)
# ---------------------------------------------------------------------------
AASHTO_SRC = ("https://www.cedengineering.com/userfiles/C08-024%20-%20Practical%20Geometric%20Design"
              "%20of%20Roadways%20-%20US.pdf")
AASHTO_STD = "AASHTO 2018 Green Book"
AASHTO_NOTE = ("Dimensions from AASHTO Green Book 2018 Table 2-4a, turning radius = 'minimum design turning "
               "radius' (path of outer front wheel) from Table 2-5a, both in US units converted (1 ft = 0.3048 m), "
               "as reproduced in CED course C08-024 Tables 1.5/1.7. ")
TR_FT = {"P": 23.8, "SU-30": 41.8, "SU-40": 51.2, "BUS-40": 41.7, "BUS-45": 44.0, "CITY-BUS": 41.6,
         "S-BUS-36": 38.6, "S-BUS-40": 39.1, "A-BUS": 39.4, "WB-40": 39.9, "WB-62": 44.8, "WB-67": 44.8,
         "WB-67D": 44.8, "WB-92D": 82.0, "WB-100T": 44.8, "WB-109D": 59.9, "MH": 39.7, "P/T": 32.9,
         "P/B": 23.8, "MH/B": 49.8}
TANDEM = 1.22  # m, assumed spread of a US tandem axle group (4 ft) - estimated


def tw(width_m):
    """track = width - 0.25 m (rule of thumb from the brief)"""
    return width_m - 0.25


def tandem(xc, track, dual=True, spread=TANDEM):
    return [axle(xc - spread / 2, track, dual), axle(xc + spread / 2, track, dual)]


def aashto_rigid(id, sym, name_en, name_nl, category, kind, H, W, L, FO, WB, extra_est=(), rear_tandem=False,
                 note=""):
    w = ft(W)
    axs = [axle(ft(FO), tw(w), False, True)]
    est = ["track"]
    if rear_tandem:
        axs += tandem(ft(FO + WB), tw(w))
        est += ["tandem_spacing", "dual"]
    else:
        dual = category not in ("car", "special") or sym == "MH"
        axs.append(axle(ft(FO + WB), tw(w), dual, False))
        if dual:
            est.append("dual")
    add(id, name_en, name_nl, category, AASHTO_STD, "US", AASHTO_SRC, None if H is None else ft(H),
        ft(TR_FT[sym]), [unit(kind, ft(L), w, axs)], est + list(extra_est), AASHTO_NOTE + note)


aashto_rigid("aashto_p", "P", "Passenger car P (AASHTO)", "Personenauto P (AASHTO)", "car", "car",
             4.3, 7.0, 19.0, 3.0, 11.0)
aashto_rigid("aashto_su30", "SU-30", "Single-unit truck SU-30 (AASHTO)", "Vrachtwagen 2-assig SU-30 (AASHTO)",
             "truck", "body", 13.5, 8.0, 30.0, 4.0, 20.0,
             note="Height given as 11.0-13.5 ft; upper value used.")
aashto_rigid("aashto_su40", "SU-40", "Single-unit truck, three-axle SU-40 (AASHTO)",
             "Vrachtwagen 3-assig SU-40 (AASHTO)", "truck", "body", 13.5, 8.0, 39.5, 4.0, 25.0, rear_tandem=True,
             note="Height given as 11.0-13.5 ft; upper value used. WB is to the centre of the rear tandem.")
aashto_rigid("aashto_bus40", "BUS-40", "Intercity bus (motor coach) BUS-40 (AASHTO)",
             "Touringcar BUS-40 (AASHTO)", "coach", "bus", 12.0, 8.5, 40.5, 6.3, 25.3,
             note="Rear overhang listed as 9.0 ft from back axle; FO+WB+9.0 = 40.6 ft vs 40.5 ft length "
                  "(AASHTO rounding), modelled as two axle groups.")
# BUS-45: rear overhang 9.0 ft measured from back axle of tandem (footnote a) -> back axle at 36.5 ft,
# effective axle at 34.7 ft -> drive axle at 32.9 ft (derived)
w = ft(8.5)
add("aashto_bus45", "Intercity bus (motor coach) BUS-45 (AASHTO)", "Touringcar BUS-45 (AASHTO)", "coach",
    AASHTO_STD, "US", AASHTO_SRC, ft(12.0), ft(TR_FT["BUS-45"]),
    [unit("bus", ft(45.5), w, [axle(ft(6.2), tw(w), False, True), axle(ft(32.9), tw(w), True, False),
                               axle(ft(36.5), tw(w), False, False)])],
    ["track", "dual"],
    AASHTO_NOTE + "WB1 28.5 ft is to the effective rear axle (34.7 ft); rear overhang 9.0 ft is from the back "
                  "(tag) axle, so the drive and tag axles are placed at 32.9 and 36.5 ft (derived).")
aashto_rigid("aashto_city_bus", "CITY-BUS", "City transit bus CITY-BUS (AASHTO)", "Stadsbus CITY-BUS (AASHTO)",
             "bus", "bus", 10.5, 8.5, 40.0, 7.0, 25.0,
             note="AASHTO SI table lists 12.80 m design turning radius; US value 41.6 ft converted is used.")
aashto_rigid("aashto_sbus36", "S-BUS-36", "Conventional school bus, 65 pass. S-BUS-36 (AASHTO)",
             "Schoolbus conventioneel S-BUS-36 (AASHTO)", "bus", "bus", 10.5, 8.0, 35.8, 2.5, 21.3)
aashto_rigid("aashto_sbus40", "S-BUS-40", "Large school bus, 84 pass. S-BUS-40 (AASHTO)",
             "Grote schoolbus S-BUS-40 (AASHTO)", "bus", "bus", 10.5, 8.0, 40.0, 7.0, 20.0)

# A-BUS: FO 8.6, WB1 22.0, S 6.2 (middle axle to articulation), T 13.2, RO 10.0, L 60.0
w = ft(8.5)
art = 8.6 + 22.0 + 6.2  # 36.8 ft
add("aashto_abus", "Articulated bus A-BUS (AASHTO)", "Gelede bus A-BUS (AASHTO)", "articulated_bus",
    AASHTO_STD, "US", AASHTO_SRC, ft(11.0), ft(TR_FT["A-BUS"]),
    [unit("bus", ft(art), w, [axle(ft(8.6), tw(w), False, True), axle(ft(30.6), tw(w), True, False)],
          hitch_x=ft(art)),
     unit("bus_rear", ft(60.0) - ft(art), w, [axle(ft(13.2), tw(w), True, False)], kingpin_x=0.0)],
    ["track", "dual"],
    AASHTO_NOTE + "Units split at the articulation point (S = 6.2 ft behind middle axle, T = 13.2 ft to rear "
                  "axle); the 4.0 ft wide articulating section is not drawn separately.")


def tractor_unit(FO, WB1, W, rear_tandem, tail_ft):
    w = ft(W)
    rear = FO + WB1
    axs = [axle(ft(FO), tw(w), False, True)]
    axs += tandem(ft(rear), tw(w)) if rear_tandem else [axle(ft(rear), tw(w), True, False)]
    return unit("tractor", ft(rear + tail_ft), w, axs, hitch_x=ft(rear))


def semi_unit(kp_ft, wb_ft, ro_ft, W, rear_tandem, hitch_after_axle_ft=None):
    w = ft(W)
    xa = kp_ft + wb_ft
    axs = tandem(ft(xa), tw(w)) if rear_tandem else [axle(ft(xa), tw(w), True, False)]
    hx = None if hitch_after_axle_ft is None else ft(xa + hitch_after_axle_ft)
    return unit("semitrailer", ft(xa + ro_ft), w, axs, hitch_x=hx, kingpin_x=ft(kp_ft))


def dolly_unit(T, W):
    w = ft(W)
    # drawbar eye at x=0, axle at T, fifth wheel over the axle
    return unit("dolly", ft(T + 1.5), w, [axle(ft(T), tw(w), True, False)], hitch_x=ft(T), kingpin_x=0.0)


TRUCK_NOTE = ("Kingpin assumed over the effective tractor drive axle (AASHTO convention: WB2 = kingpin to "
              "trailer axle group). Tractor tail length, tandem spread (1.22 m) and twin tyres are estimates. ")

# WB-40: trailer length not given -> kingpin_x 3.0 ft estimated (gives 33 ft trailer)
add("aashto_wb40", "Intermediate semitrailer WB-40 (AASHTO)", "Trekker met oplegger WB-40 (AASHTO)",
    "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5), ft(TR_FT["WB-40"]),
    [tractor_unit(3.0, 12.5, 8.0, False, 2.0), semi_unit(3.0, 25.5, 4.5, 8.0, True)],
    ["track", "dual", "tandem_spacing", "tractor_length", "kingpin_x"],
    AASHTO_NOTE + TRUCK_NOTE + "Semitrailer front overhang ahead of kingpin (3.0 ft) estimated; overall 45.5 ft.")
# WB-62: 48 ft trailer (STAA) -> trailer front at 69-48 = 21.0 ft, kingpin at 23.5 ft -> kingpin_x 2.5 ft
add("aashto_wb62", "Interstate semitrailer WB-62 (AASHTO)", "Trekker met oplegger WB-62 (AASHTO)",
    "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5), ft(TR_FT["WB-62"]),
    [tractor_unit(4.0, 19.5, 8.5, True, 3.0), semi_unit(2.5, 41.0, 4.5, 8.5, True)],
    ["track", "dual", "tandem_spacing", "tractor_length"],
    AASHTO_NOTE + TRUCK_NOTE + "48.0 ft trailer (1982 STAA); kingpin_x 2.5 ft derived from overall length 69.0 ft.")
add("aashto_wb67", "Interstate semitrailer WB-67 (AASHTO)", "Trekker met oplegger WB-67 (AASHTO)",
    "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5), ft(TR_FT["WB-67"]),
    [tractor_unit(4.0, 19.5, 8.5, True, 3.0), semi_unit(3.0, 45.5, 4.5, 8.5, True)],
    ["track", "dual", "tandem_spacing", "tractor_length"],
    AASHTO_NOTE + TRUCK_NOTE + "53.0 ft trailer (STAA grandfathered); kingpin_x 3.0 ft derived from overall length "
                               "73.5 ft. AASHTO advises WB-62 for turning where 41 ft kingpin-to-axle is common.")

DBL_NOTE = ("Trailer front overhang ahead of kingpin and dolly length estimated; trailer rear ends at the "
            "pintle hitch (S behind axle). Assembled units are ~0.5-1.0 ft shorter than the AASHTO overall "
            "length due to rounding in the AASHTO table. ")
add("aashto_wb67d", "Double-bottom semitrailer/trailer WB-67D (AASHTO)",
    "Trekker met twee opleggers (double bottom) WB-67D (AASHTO)", "tractor_semitrailer", AASHTO_STD, "US",
    AASHTO_SRC, ft(13.5), ft(TR_FT["WB-67D"]),
    [tractor_unit(2.3, 11.0, 8.5, False, 2.0), semi_unit(3.0, 23.0, 3.0, 8.5, False, hitch_after_axle_ft=3.0),
     dolly_unit(7.0, 8.5), semi_unit(3.0, 22.5, 3.0, 8.5, False)],
    ["track", "dual", "tractor_length", "kingpin_x", "dolly_length"],
    AASHTO_NOTE + TRUCK_NOTE + DBL_NOTE + "WB1 11.0, WB2 23.0, S 3.0, T 7.0, WB3 22.5 ft; overall 72.3 ft.")
add("aashto_wb92d", "Rocky Mountain double-semitrailer/trailer WB-92D (AASHTO)",
    "Rocky Mountain double WB-92D (AASHTO)", "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5),
    ft(TR_FT["WB-92D"]),
    [tractor_unit(2.3, 17.5, 8.5, True, 3.0), semi_unit(3.0, 40.0, 4.5, 8.5, True, hitch_after_axle_ft=4.5),
     dolly_unit(7.0, 8.5), semi_unit(3.0, 22.5, 3.0, 8.5, False)],
    ["track", "dual", "tandem_spacing", "tractor_length", "kingpin_x", "dolly_length"],
    AASHTO_NOTE + TRUCK_NOTE + DBL_NOTE + "WB1 17.5, WB2 40.0, S 4.5, T 7.0, WB3 22.5 ft; overall 97.3 ft.")
add("aashto_wb100t", "Triple semitrailer/trailers WB-100T (AASHTO)", "Trekker met drie opleggers WB-100T (AASHTO)",
    "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5), ft(TR_FT["WB-100T"]),
    [tractor_unit(2.3, 11.0, 8.5, False, 2.0), semi_unit(3.0, 22.5, 3.0, 8.5, False, hitch_after_axle_ft=3.0),
     dolly_unit(7.0, 8.5), semi_unit(3.0, 22.5, 3.0, 8.5, False, hitch_after_axle_ft=3.0),
     dolly_unit(7.0, 8.5), semi_unit(3.0, 22.5, 3.0, 8.5, False)],
    ["track", "dual", "tractor_length", "kingpin_x", "dolly_length"],
    AASHTO_NOTE + TRUCK_NOTE + DBL_NOTE + "WB1 11.0, WB2 22.5, S 3.0, T 7.0, WB3 22.5, WB4 22.5 ft (second S/T "
                                          "taken equal to the first); overall 104.8 ft.")
# WB-109D: 48 ft trailers; rear trailer RO 4.5 -> kingpin_x = 48-40-4.5 = 3.5 ft (derived)
add("aashto_wb109d", "Turnpike double-semitrailer/trailer WB-109D (AASHTO)",
    "Turnpike double WB-109D (AASHTO)", "tractor_semitrailer", AASHTO_STD, "US", AASHTO_SRC, ft(13.5),
    ft(TR_FT["WB-109D"]),
    [tractor_unit(2.3, 12.2, 8.5, True, 3.0), semi_unit(3.5, 40.0, 4.5, 8.5, True, hitch_after_axle_ft=4.5),
     dolly_unit(10.0, 8.5), semi_unit(3.5, 40.0, 4.5, 8.5, True)],
    ["track", "dual", "tandem_spacing", "tractor_length", "dolly_length"],
    AASHTO_NOTE + TRUCK_NOTE + DBL_NOTE + "Two 48.0 ft trailers (STAA); kingpin_x 3.5 ft derived from trailer "
                                          "length, WB 40.0 and RO 4.5 ft. WB1 12.2, S 4.5, T 10.0 ft; overall 114.0 ft.")

aashto_rigid("aashto_mh", "MH", "Motor home MH (AASHTO)", "Camper MH (AASHTO)", "special", "body",
             12.0, 8.0, 30.0, 4.0, 20.0)


def car_trailer(id, sym, name_en, name_nl, carH, trH, carFO, carWB, carL, carW, S, T, RO, trW, front_kind,
                front_steer_dual=False, note=""):
    cw = ft(carW)
    tw_ = ft(trW)
    hitch = carFO + carWB + S
    front = unit(front_kind, ft(carL), cw, [axle(ft(carFO), tw(cw), False, True),
                                           axle(ft(carFO + carWB), tw(cw), front_steer_dual, False)],
                 hitch_x=ft(hitch))
    trailer = unit("trailer", ft(T + RO), tw_, [axle(ft(T), tw(tw_), False, False)], kingpin_x=0.0)
    H = None if trH is None else ft(max(carH, trH))
    add(id, name_en, name_nl, "special", AASHTO_STD, "US", AASHTO_SRC, H, ft(TR_FT[sym]), [front, trailer],
        ["track"] + (["dual"] if front_steer_dual else []),
        AASHTO_NOTE + "Trailer unit is measured from the coupling point (kingpin_x = 0) and includes the "
                      "drawbar/tongue: hitch-to-axle T and rear overhang from the table. " + note)


car_trailer("aashto_pt", "P/T", "Car and camper trailer P/T (AASHTO)", "Auto met caravan P/T (AASHTO)",
            4.3, 10.0, 3.0, 11.0, 19.0, 7.0, 5.0, 17.7, 12.0, 8.0, "car",
            note="S 5.0 ft, T 17.7 ft, overall 48.7 ft.")
car_trailer("aashto_pb", "P/B", "Car and boat trailer P/B (AASHTO)", "Auto met boottrailer P/B (AASHTO)",
            4.3, None, 3.0, 11.0, 19.0, 7.0, 5.0, 15.0, 8.0, 8.0, "car",
            note="AASHTO gives no height for P/B. S 5.0, T 15.0 ft, overall 42.0 ft.")
car_trailer("aashto_mhb", "MH/B", "Motor home and boat trailer MH/B (AASHTO)", "Camper met boottrailer MH/B (AASHTO)",
            12.0, 12.0, 4.0, 20.0, 30.0, 8.0, 6.0, 15.0, 8.0, 8.0, "body", front_steer_dual=True,
            note="S 6.0, T 15.0 ft, overall 53.0 ft.")

# ---------------------------------------------------------------------------
# 2. Germany - FGSV / RBSV
# ---------------------------------------------------------------------------
WHZ = "https://www.whz.de/fileadmin/fakultaeten/kft/docs/organisation/iev/forschung/projekte/FoBemPkw.pdf"
RAST_K = "https://www.fgsv-verlag.de/pub/media/pdf/200.k.pdf"
HERNE = ("https://www.herne.de/PDF/Bauleitplanung/bp257_4/Anlage-3.7_BP-257_Erl%C3%A4uterungsbericht-"
         "Befahrbarkeit.pdf")
MEDEBACH = "https://www.medebach.de/fileadmin/user_upload/Stadt/Ausschreibungen/2020-12-08-Befahrbarkeitsuntersuchung.pdf"
FGSV_STD = "FGSV RASt 06 / Bemessungsfahrzeuge"

add("fgsv_pkw_2001", "Design car Pkw (FGSV 2001)", "Ontwerpauto Pkw (FGSV 2001)", "car", FGSV_STD, "DE", WHZ,
    1.51, None,
    [unit("car", 4.74, 1.76, [axle(0.94, tw(1.76), False, True), axle(0.94 + 2.70, tw(1.76))])],
    ["track"],
    "85th-percentile design car of FGSV 'Bemessungsfahrzeuge und Schleppkurven' (2001): L 4.74, WB 2.70, "
    "H 1.51, W 1.76 (without mirrors), as cited in WHZ/TU Braunschweig report 'Bemessungs-Pkw' (Table 3). "
    "Front overhang 0.94 derived from RASt 06 Tabelle 21 'Deichselmass' D = WB + front overhang = 3.64 m "
    "(" + RAST_K + "). Turning radius not given in the opened sources.")
add("fgsv_pkw_2010_proposal", "Design car Pkw 2010 proposal (WHZ/TU Braunschweig)",
    "Ontwerpauto Pkw 2010-voorstel (WHZ/TU Braunschweig)", "car", "generic", "DE", WHZ, 1.76, 12.14 / 2,
    [unit("car", 4.93, 1.91, [axle(0.99, tw(1.91), False, True), axle(0.99 + 2.85, tw(1.91))])],
    ["track"],
    "Updated 85th-percentile design car (unweighted by sales), Abbildung 4 of the report: 0.99 + 2.85 + 1.09 = "
    "4.93 m, width 1.91 m; height 1.76 m = 85th percentile 2010 (Tabelle 3). Turning radius = half of the "
    "85th-percentile outer turning-circle diameter 12.14 m (fleet statistic, not of this exact vehicle). "
    "Research proposal feeding into RBSV 2020, not the official FGSV vehicle.")
add("rbsv_pkw_2020", "Design car Pkw (RBSV 2020)", "Ontwerpauto Pkw (RBSV 2020)", "car", FGSV_STD, "DE", HERNE,
    None, 5.85,
    [unit("car", 4.88, 1.90, [axle(0.92, tw(1.90), False, True), axle(0.92 + 2.86, tw(1.90))])],
    ["track", "width"],
    "FGSV RBSV 2020 design car as shown in the vehicle data block of a Befahrbarkeitsuntersuchung (Herne B-Plan "
    "257): 0.92 + 2.86 + 1.10 = 4.88 m, R_aussen = 5.85 m. Width not stated there: 1.90 m taken from the "
    "WHZ 85th-percentile 2010 car (estimate). Height unknown.")
add("fgsv_sattelzug_2001", "Tractor-semitrailer Sattelzug (FGSV 2001)", "Trekker met oplegger Sattelzug (FGSV 2001)",
    "tractor_semitrailer", FGSV_STD, "DE", MEDEBACH, 4.00, 7.90,
    [unit("tractor", 6.08, 2.55, [axle(1.43, tw(2.55), False, True), axle(5.23, tw(2.55), True, False)],
          hitch_x=4.50),
     unit("semitrailer", 13.61, 2.55, [axle(9.36 - 1.31, tw(2.55)), axle(9.36, tw(2.55)), axle(9.36 + 1.31, tw(2.55))],
          kingpin_x=1.61)],
    ["track", "width", "height", "tandem_spacing", "dual"],
    "Vehicle data block 'Bemessungsfahrzeug nach FGSV 2001 - Sattelzug' in a Befahrbarkeitsuntersuchung "
    "(Medebach 2020): tractor 1.43 + 3.80 + 0.85 (= 6.08 m), kingpin 1.58 m ahead of tractor rear end, "
    "semitrailer 1.61 + 7.75 + 4.25 (= 13.61 m), overall 16.50 m, R_aussen = 7.90 m. Tri-axle spacing (1.31 m) "
    "estimated; width 2.55 and height 4.00 are the legal maxima (StVZO / 96/53/EC), not read from the block.")

# Stadtbahnwagen B (DE light rail)
add("de_stadtbahnwagen_b", "Light rail vehicle Stadtbahnwagen B (DUEWAG, 28 m)",
    "Lightrailvoertuig Stadtbahnwagen B (DUEWAG, 28 m)", "rail", "manufacturer", "DE",
    "https://de.wikipedia.org/wiki/Stadtbahnwagen_Typ_B", 3.36, None,
    [unit("rail_car", 14.0, 2.65, [axle(4.0, 1.435, False, True)], hitch_x=14.0),
     unit("rail_car", 14.0, 2.65, [axle(0.0, 1.435, False, True), axle(10.0, 1.435, False, True)], kingpin_x=0.0)],
    ["axles", "hitch_x"],
    "Two-section articulated LRV (B'2'B'), length over couplers 28.000 m, width 2.650 m, height 3.360 m, bogie "
    "pivot distance 10.000 m, bogie wheelbase 2.100 m, standard gauge 1435 mm (track = gauge). Axles here are "
    "bogie centres; sections assumed equal (14.0 m each) with the Jakobs bogie at the articulation (x = 0 of "
    "section 2), so bogie positions are derived/estimated. Used in Koeln, Bonn, Duesseldorf, Essen, etc.")

# ---------------------------------------------------------------------------
# 3. EU 96/53/EC maxima with axle layouts of Kanton Zuerich templates
# ---------------------------------------------------------------------------
EU_SRC = "https://www.legislation.gov.uk/eudr/1996/53/annex/I"
ZH = ("https://www.zh.ch/content/dam/zhweb/bilder-dokumente/themen/planen-bauen/tiefbau/dokumente-tiefbau/"
      "kreisel/grundlagen_schleppkurvennachweise.pdf")
EU_STD = "EU 96/53/EC (max)"
EU_NOTE = ("Max. dimensions per Directive 96/53/EC Annex I as amended (2015/719, 2019/1242): width 2.55 m "
           "(2.60 m conditioned bodies), height 4.00 m, swept circle outer radius 12.50 m / inner 5.30 m "
           "(turning_radius_outer = 12.50). Axle layout from Kanton Zuerich Tiefbauamt 'Grundlagen "
           "Schleppkurvennachweise' (" + ZH + "), which models vehicles at these limits. ")

add("eu_tractor_semitrailer_16_5", "Articulated vehicle 16.50 m (EU max.)", "Trekker met oplegger 16,50 m (EU-max.)",
    "tractor_semitrailer", EU_STD, "EU", EU_SRC, 4.00, 12.50,
    [unit("tractor", 6.05, 2.55, [axle(1.25, tw(2.55), False, True), axle(5.20, tw(2.55), True, False)],
          hitch_x=4.41),
     unit("semitrailer", 14.16, 2.55, [axle(9.73 - 1.31, tw(2.55)), axle(9.73, tw(2.55)), axle(9.73 + 1.31, tw(2.55))],
          kingpin_x=2.07)],
    ["track", "tractor_length", "tandem_spacing", "dual"],
    EU_NOTE + "Zuerich template: tractor front overhang 1.25, WB 3.95, fifth wheel 0.79 ahead of drive axle; "
              "semitrailer 14.16 m, kingpin to tri-axle centre 7.66, rear overhang 4.43 (kingpin_x 2.07 derived). "
              "Tractor rear end (0.85 m behind drive axle) and tri-axle spacing 1.31 m estimated.")
add("eu_truck_trailer_18_75", "Road train (truck + drawbar trailer) 18.75 m (EU max.)",
    "Vrachtwagen met aanhanger 18,75 m (EU-max.)", "truck_trailer", EU_STD, "EU", EU_SRC, 4.00, 12.50,
    [unit("body", 9.90, 2.55, [axle(1.80, tw(2.55), False, True), axle(8.00 - 0.675, tw(2.55), True, False),
                              axle(8.00 + 0.675, tw(2.55), True, False)], hitch_x=9.60),
     unit("trailer", 7.55, 2.55, [axle(1.20, tw(2.55), False, True), axle(6.35, tw(2.55))], kingpin_x=-1.60)],
    ["track", "tandem_spacing", "dual"],
    EU_NOTE + "Zuerich template (theoretical composition): 3-axle rigid 9.90 m (front overhang 1.80, front axle "
              "to tandem centre 6.20, coupling 1.60 behind tandem centre), gap 1.30 m, 2-axle drawbar trailer "
              "7.55 m (1.20 + 5.15 + 1.20) with drawbar 2.80 m from coupling to trailer front axle. kingpin_x is "
              "negative: drawbar eye lies 1.60 m ahead of the trailer body. Tandem spacing 1.35 m estimated.")
add("eu_articulated_bus_18_75", "Articulated bus 18.75 m (EU max.)", "Gelede bus 18,75 m (EU-max.)",
    "articulated_bus", EU_STD, "EU", EU_SRC, None, 12.50,
    [unit("bus", 10.60, 2.55, [axle(2.77, tw(2.55), False, True), axle(8.70, tw(2.55), True, False)], hitch_x=10.60),
     unit("bus_rear", 8.15, 2.55, [axle(4.94, tw(2.55), True, False)], kingpin_x=0.0)],
    ["track", "dual"],
    EU_NOTE + "Zuerich 'Gelenkbus' (rigid trailing axle, pusher): 2.77 + 5.93 + 1.90 (middle axle to joint) + "
              "4.94 (joint to rear axle) + 3.21 = 18.75 m. Units split at the joint. Height not given.")
add("eu_bus_3axle_15", "Three-axle bus 15 m (EU max.)", "Driedubbelassige bus 15 m (EU-max.)", "bus", EU_STD, "EU",
    EU_SRC, None, 12.50,
    [unit("bus", 14.95, 2.55, [axle(3.10, tw(2.55), False, True), axle(10.05, tw(2.55), True, False),
                              axle(11.55, tw(2.55), False, True)])],
    ["track", "dual", "tag_axle_x"],
    EU_NOTE + "Zuerich 'Dreiachsbus (15 m)': front overhang 3.10, WB 6.95 to drive axle, 4.90 from drive axle to "
              "rear (drawn length 14.95 m, table 15.00 m). Tag axle is force-steered; its position (1.50 m behind "
              "the drive axle) is estimated. Height not given.")
add("eu_bus_2axle_12", "Standard city bus 12 m (EU/CH template)", "Standaard stadsbus 12 m (EU/CH-sjabloon)", "bus",
    EU_STD, "EU", EU_SRC, None, 12.50,
    [unit("bus", 12.14, 2.55, [axle(2.81, tw(2.55), False, True), axle(2.81 + 5.90, tw(2.55), True, False)])],
    ["track", "dual"],
    EU_NOTE + "Zuerich 'Normalbus (12 m)': 2.81 + 5.90 + 3.43 = 12.14 m (EU limit for 2-axle buses is 13.50 m). "
              "Height not given.")

# ---------------------------------------------------------------------------
# 4. Austroads (only the vehicle whose drawing was read)
# ---------------------------------------------------------------------------
add("austroads_car_b99", "Passenger vehicle 5.2 m (Austroads B99)", "Personenauto 5,2 m (Austroads B99)", "car",
    "Austroads", "AU",
    "http://web.archive.org/web/20221025003447/https://austroads.com.au/__data/assets/file/0027/395352/"
    "AP-G94-13_Design_Vehicles.zip", None, 6.3,
    [unit("car", 5.20, 1.94, [axle(0.95, tw(1.94), False, True), axle(0.95 + 3.05, tw(1.94))])],
    ["track", "front_overhang"],
    "Austroads AP-G34-13 'Design Vehicles and Turning Path Templates' (2013), drawing 'Passenger vehicle "
    "(5.2 m)': overall 5200, overhangs 950 / 1200, WB 3050, width 1940 mm. The 950 mm overhang is taken as the "
    "front one (rounded nose in the drawing) - orientation inferred. Turning radius = smallest Austroads "
    "template radius for this vehicle (6.3 m). Height not given.")

# ---------------------------------------------------------------------------
# 5. UK
# ---------------------------------------------------------------------------
LST = ("https://www.gov.uk/government/publications/longer-semi-trailer-lst-implementation-guide-for-operators/"
       "longer-semi-trailers-full-implementation-guide-for-operators")
add("uk_longer_semitrailer_18_55", "Longer semi-trailer combination 18.55 m (UK LST)",
    "Trekker met verlengde oplegger 18,55 m (VK LST)", "tractor_semitrailer", "UK", "UK", LST, 4.00, None,
    [unit("tractor", 6.05, 2.55, [axle(1.25, tw(2.55), False, True), axle(5.20, tw(2.55), True, False)],
          hitch_x=4.41),
     unit("semitrailer", 15.65, 2.55, [axle(11.30, tw(2.55)), axle(12.61, tw(2.55)), axle(13.92, tw(2.55), False, True)],
          kingpin_x=1.60)],
    ["track", "tractor_length", "axles", "width", "height", "dual"],
    "UK longer semi-trailer (LST): semi-trailer up to 15.65 m, combination max 18.55 m, kingpin to rear "
    "<= 14.05 m (kingpin_x 1.60 derived at that limit), at least 3 axles of which one steers, must pass the "
    "13.6 m trailer turning-circle test. Tractor taken from the EU 16.50 m template; trailer axle positions "
    "(tri-axle, rear axle steered) estimated; assembled length 18.46 m. Width/height = general limits.")

LEVC = "https://en.wikipedia.org/wiki/LEVC_TX"
add("uk_taxi_levc_tx", "London taxi LEVC TX", "Londense taxi LEVC TX", "car", "manufacturer", "UK", LEVC, 1.888,
    4.23,
    [unit("car", 4.857, 1.874, [axle(0.94, tw(1.874), False, True), axle(0.94 + 2.986, tw(1.874))])],
    ["track", "front_overhang"],
    "L 4857, W 1874, H 1888, WB 2986 mm; turning circle 8.45 m (radius 4.23 m) per London taxi conditions "
    "of fitness (front wheels steer up to 63 deg). Overhang split estimated (equal).")

LTN = "https://assets.publishing.service.gov.uk/media/5ffa1f96d3bf7f65d9e35825/cycle-infrastructure-design-ltn-1-20.pdf"
LTN_NOTE = ("DfT LTN 1/20 Cycle Infrastructure Design, Table 5-1 (typical length/width, minimum turning circle "
            "outer radius). Wheel positions are not in the source and are estimated. ")
add("uk_bicycle", "Solo upright cycle (LTN 1/20)", "Fiets (LTN 1/20)", "two_wheeler", "UK", "UK", LTN, None, 1.65,
    [unit("bike", 1.80, 0.65, [axle(0.35, 0.0, False, True), axle(1.45, 0.0)])], ["axles"],
    LTN_NOTE + "Width is handlebar width. Inner turning radius 0.85 m.")
add("uk_tandem_cycle", "Tandem cycle (LTN 1/20)", "Tandem (LTN 1/20)", "two_wheeler", "UK", "UK", LTN, None, 3.15,
    [unit("bike", 2.40, 0.65, [axle(0.35, 0.0, False, True), axle(2.05, 0.0)])], ["axles"],
    LTN_NOTE + "Inner turning radius 2.25 m.")
add("uk_cycle_with_trailer", "Cycle with 850 mm trailer (LTN 1/20)", "Fiets met aanhanger 850 mm (LTN 1/20)",
    "two_wheeler", "UK", "UK", LTN, None, 2.65,
    [unit("bike", 1.80, 0.65, [axle(0.35, 0.0, False, True), axle(1.45, 0.0)], hitch_x=1.60),
     unit("trailer", 1.10, 0.85, [axle(0.75, 0.60)], kingpin_x=0.0)],
    ["axles", "hitch_x", "track"],
    LTN_NOTE + "Combination 2.7 m long, 0.85 m wide; trailer unit measured from the coupling.")
add("uk_cycle_design_vehicle", "Cycle design vehicle / cargo bike envelope (LTN 1/20)",
    "Ontwerpfiets / bakfiets-omhullende (LTN 1/20)", "two_wheeler", "UK", "UK", LTN, None, 3.4,
    [unit("bike", 2.80, 1.20, [axle(0.35, 0.0, False, True), axle(2.45, 0.90)])], ["axles", "track"],
    LTN_NOTE + "Composite 'cycle design vehicle' (max 2.8 m x 1.2 m) covering cargo bikes, tricycles and adapted "
               "cycles; use as bakfiets envelope. Inner radius 0.1 m (pivoting cycles) or 2.5 m (3/4-wheel cycles).")

IM = ("https://assets.publishing.service.gov.uk/media/61d32bb7d3bf7f1f72b5ffd2/inclusive-mobility-a-guide-to-best-"
      "practice-on-access-to-pedestrian-and-transport-infrastructure.pdf")
add("uk_wheelchair_manual", "Manual wheelchair with user, 95th percentile (Inclusive Mobility)",
    "Handbewogen rolstoel met gebruiker, 95e percentiel (Inclusive Mobility)", "special", "UK", "UK", IM, 1.374, None,
    [unit("body", 1.183, 0.702, [axle(0.15, 0.55, False, True), axle(0.85, 0.65)])], ["axles", "track"],
    "DfT Inclusive Mobility (2021), TRL 1999 occupied-wheelchair survey: manual wheelchair 95th percentile "
    "length 1183 mm, width 702 mm (excl. elbows/hands); height = 95th percentile of all wheelchair users "
    "(1374 mm). Castor/drive-wheel positions estimated. Manual propulsion needs 50-100 mm clearance each side.")
add("uk_mobility_scooter", "Mobility scooter with user, 95th percentile (Inclusive Mobility)",
    "Scootmobiel met gebruiker, 95e percentiel (Inclusive Mobility)", "special", "UK", "UK", IM, 1.438, None,
    [unit("body", 1.402, 0.685, [axle(0.25, 0.45, False, True), axle(1.15, 0.55)])], ["axles", "track"],
    "DfT Inclusive Mobility (2021): mobility scooter 95th percentile length 1402 mm, width 685 mm, height "
    "1438 mm (95th percentile of scooter users). Wheel positions estimated.")

# ---------------------------------------------------------------------------
# 6. Generic / manufacturer vehicles
# ---------------------------------------------------------------------------
AD = "https://www.auto-data.net/en/"


def car(id, name_en, name_nl, category, region, url, L, W, H, WB, FO=None, tf=None, tr=None, tc=None, kind="car",
        dual_rear=False, note="", turning_is_wall=False):
    est = []
    if FO is None:
        FO = (L - WB) / 2
        est.append("front_overhang")
    if tf is None:
        tf = tw(W)
        tr = tw(W)
        est.append("track")
    if tr is None:
        tr = tf
    tr_out = None if tc is None else tc / 2
    n = note
    if tc is not None:
        n += (" Turning radius = half the published turning circle diameter (%.2f m%s)."
              % (tc, ", wall to wall" if turning_is_wall else "; curb/wall basis not stated"))
    if "front_overhang" in est:
        n += " Overhang split not published: front/rear taken equal."
    add(id, name_en, name_nl, category, "manufacturer", region, url, H, tr_out,
        [unit(kind, L, W, [axle(FO, tf, False, True), axle(FO + WB, tr, dual_rear, False)])], est, n.strip())


car("car_fiat_500e", "City car Fiat 500e (332)", "Stadsauto Fiat 500e (332)", "car", "EU",
    AD + "fiat-500-332-3-1-e-42-kwh-118hp-42061", 3.632, 1.683, 1.527, 2.322, tc=9.7,
    note="Mini segment (A). Width without mirrors (1900 mm with mirrors).")
car("car_vw_polo", "Small car VW Polo VI", "Kleine auto VW Polo VI", "car", "EU",
    AD + "volkswagen-polo-vi-facelift-2021-1.0-tsi-116hp-dsg-51660", 4.074, 1.751, 1.451, 2.552, tf=1.500, tr=1.483,
    tc=10.7, note="Small car (B segment), 2021 facelift. Width without mirrors (1964 mm with).")
car("car_renault_clio", "Small car Renault Clio VI", "Kleine auto Renault Clio VI", "car", "EU",
    AD + "renault-clio-vi-phase-i-1.8-158hp-e-tech-full-hybrid-multimode-55302", 4.116, 1.768, 1.451, 2.591,
    FO=0.859, tf=1.503, tr=1.503, tc=10.4, note="B segment. Published rear overhang 0.667 m (sum 4.117 m).")
car("car_vw_golf", "Compact car VW Golf 8", "Middenklasse VW Golf 8", "car", "EU",
    AD + "volkswagen-golf-viii-facelift-2024-1.5-etsi-150hp-mild-hybrid-dsg-51252", 4.282, 1.789, 1.483, 2.620,
    tf=1.534, tr=1.502, tc=10.9, note="C segment hatchback, 2024 facelift. Width without mirrors (2073 mm with).")
car("car_toyota_corolla", "Compact sedan Toyota Corolla (E210)", "Middenklasse sedan Toyota Corolla (E210)", "car",
    "GLOBAL", AD + "toyota-corolla-xii-e210-facelift-2022-1.8-140hp-hybrid-e-cvt-48728", 4.630, 1.780, 1.435, 2.700,
    FO=0.935, tf=1.531, tr=1.544, tc=10.4, note="World's best-selling car line; rear overhang 0.995 m.")
car("car_toyota_camry", "Mid-size sedan Toyota Camry (XV70)", "Middenklasse sedan Toyota Camry (XV70)", "car",
    "US", AD + "toyota-camry-viii-xv70-facelift-2020-xse-2.5-205hp-awd-ect-i-50523", 4.895, 1.840, 1.450, 2.825,
    tf=1.580, tr=1.595, tc=12.0, note="Typical US mid-size sedan.")
car("car_mercedes_e_class", "Large sedan Mercedes-Benz E-Class (W214)", "Grote sedan Mercedes-Benz E-Klasse (W214)",
    "car", "EU", AD + "mercedes-benz-e-class-w214-e-200-204hp-mild-hybrid-9g-tronic-48455", 4.949, 1.880, 1.468,
    2.961, tf=1.634, tr=1.648, tc=11.6, note="Executive (E) segment; common taxi in DE.")
car("car_mercedes_s_class", "Luxury sedan Mercedes-Benz S-Class (W223)",
    "Luxe sedan Mercedes-Benz S-Klasse (W223)", "car", "EU",
    AD + "mercedes-benz-s-class-w223-facelift-2026-s-580e-585hp-eq-hybrid-4matic-9g-tronic-56284", 5.194, 1.921,
    1.503, 3.106, tf=1.655, tr=1.651, tc=11.6, note="F segment, normal wheelbase.")
car("car_taxi_toyota_prius", "Taxi Toyota Prius (XW60)", "Taxi Toyota Prius (XW60)", "car", "GLOBAL",
    AD + "toyota-prius-v-xw60-1.8-140hp-hybrid-e-cvt-48348", 4.600, 1.780, 1.420, 2.750, tf=1.560, tr=1.570, tc=10.6,
    note="Hybrid widely used as taxi (EU/US/JP).")
car("car_suv_toyota_rav4", "SUV Toyota RAV4 (XA50)", "SUV Toyota RAV4 (XA50)", "car", "GLOBAL",
    AD + "toyota-rav4-v-facelift-2021-2.0-173hp-cvt-54078", 4.600, 1.855, 1.690, 2.690, FO=0.910, tf=1.600,
    tr=1.630, tc=11.4, note="Compact SUV, best-selling SUV worldwide; rear overhang 1.000 m.")
car("car_suv_tesla_model_y", "SUV Tesla Model Y (2025)", "SUV Tesla Model Y (2025)", "car", "GLOBAL",
    AD + "tesla-model-y-juniper-facelift-2025-performance-84.7-kwh-460hp-dual-motor-awd-56053", 4.796, 1.920, 1.611,
    2.890, FO=0.895, tf=1.636, tr=1.622, tc=12.13, note="Mid-size electric SUV; rear overhang 1.011 m.")
car("car_suv_volvo_xc90", "Large SUV Volvo XC90 II", "Grote SUV Volvo XC90 II", "car", "EU",
    AD + "volvo-xc90-ii-facelift-2024-2.0-t8-461hp-plug-in-hybrid-awd-geartronic-57843", 4.953, 1.931, 1.771, 2.984,
    FO=0.890, tf=1.665, tr=1.667, tc=12.1, note="Rear overhang 1.079 m; turning circle 11.9-12.1 m, upper used.")
car("car_suv_chevrolet_suburban", "Full-size SUV Chevrolet Suburban (GMT T1XX)",
    "Fullsize SUV Chevrolet Suburban (GMT T1XX)", "car", "US",
    AD + "chevrolet-suburban-gmtt1xx-6.2-v8-420hp-awd-automatic-38989", 5.732, 2.059, 1.923, 3.407, tf=1.741,
    tr=1.734, note="Large US SUV (also basis of many US police/fleet vehicles). Turning circle not published.")
car("car_mpv_vw_touran", "MPV VW Touran II", "MPV VW Touran II", "car", "EU",
    AD + "volkswagen-touran-ii-2.0-tdi-scr-122hp-41682", 4.527, 1.829, 1.674, 2.786, tf=1.561, tr=1.534, tc=11.5,
    note="Compact MPV. Width without mirrors (2087 mm with).")
car("car_mpv_nissan_e_nv200", "MPV / taxi van Nissan e-NV200", "MPV / taxibus Nissan e-NV200", "car", "GLOBAL",
    AD + "nissan-e-nv200-40-kwh-109hp-32727", 4.560, 1.755, 1.858, 2.725, FO=0.985, tf=1.530, tr=1.530, tc=11.13,
    note="NV200 family is the NYC 'Taxi of Tomorrow' base; rear overhang 0.850 m.")
car("emergency_police_ford_explorer", "Police SUV Ford Explorer VI (Police Interceptor Utility base)",
    "Politie-SUV Ford Explorer VI (basis Police Interceptor Utility)", "emergency", "US",
    AD + "ford-explorer-vi-facelift-2024-2.3-ecoboost-300hp-automatic-51997", 5.047, 2.004, 1.773, 3.025, FO=0.853,
    tf=1.699, tr=1.699,
    note="Civilian Explorer VI (2024) dimensions; the Ford Police Interceptor Utility is derived from it (light bar "
         "adds height). Rear overhang 1.169 m. Turning circle not published.")
car("pickup_toyota_hilux", "Pick-up Toyota Hilux Double Cab", "Pick-up Toyota Hilux dubbele cabine", "pickup",
    "GLOBAL", AD + "toyota-hilux-double-cab-viii-facelift-2020-2.4-d-4d-150hp-4x4-41944", 5.325, 1.855, 1.815, 3.085,
    tf=1.535, tr=1.550, tc=13.4, note="Global mid-size pick-up. Width 1855-1900 mm depending on version.")
car("pickup_ford_f150", "Full-size pick-up Ford F-150 SuperCrew", "Fullsize pick-up Ford F-150 SuperCrew", "pickup",
    "US", AD + "ford-f-series-f-150-xiv-supercrew-facelift-2023-tremor-5.0-v8-400hp-4x4-selectshift-53990", 5.885,
    2.029, 2.014, 3.698, FO=0.955, tf=1.740, tr=1.755, tc=14.6,
    note="Best-selling US vehicle; SuperCrew 5.5 ft box (Tremor trim). Rear overhang 1.232 m; width without "
         "mirrors (2431 mm with).")

car("pickup_ram_1500_crew_cab", "Full-size pick-up RAM 1500 Crew Cab (DT)", "Fullsize pick-up RAM 1500 Crew Cab (DT)",
    "pickup", "US", AD + "ram-1500-crew-cab-dt-facelift-2024-5.7-hemi-v8-etorque-395hp-torqueflite-55280", 5.903, 2.062,
    1.968, 3.672, tf=1.740, tr=1.727, tc=14.08, note="Crew Cab, 5'7\" box, 2024 facelift.")

VW_BROCH = "http://blog.le-parnass.com/catalogue_pdf/vw_new_transporter201512_e.pdf"
car("van_vw_transporter_t6_swb", "Small van VW Transporter T6 SWB", "Kleine bestelbus VW Transporter T6 kort", "van",
    "EU", VW_BROCH, 4.892, 1.904, 1.990, 3.000, FO=0.896, tc=11.9, kind="body", turning_is_wall=True,
    note="VW Commercial Vehicles brochure 12/2015, panel van dimensions: 896 + 3000 + 996 = 4892 mm, width 1904 "
         "(2283 incl. mirrors), height 1990 (low roof; 2176 medium).")
car("van_vw_transporter_t61_lwb", "Van VW Transporter T6.1 LWB", "Bestelbus VW Transporter T6.1 lang", "van", "EU",
    AD + "volkswagen-transporter-t6.1-facelift-2019-panel-van-2.0-tdi-150hp-l2h1-49763", 5.304, 1.904, 1.990, 3.400,
    FO=0.906, tc=13.2, kind="body", note="L2H1 panel van; rear overhang 0.998 m.")
car("van_mercedes_vito_long", "Van Mercedes-Benz Vito long (W447)", "Bestelbus Mercedes-Benz Vito lang (W447)", "van",
    "EU", AD + "mercedes-benz-vito-w447-facelift-2023-crew-van-long-124-cdi-237hp-9g-tronic-55940", 5.140, 1.928,
    1.910, 3.200, FO=0.895, tf=1.666, tr=1.646, tc=11.8, kind="body", note="Rear overhang 1.045 m.")
car("van_toyota_hiace_super_long", "Van Toyota HiAce Super Long (H200)", "Bestelbus Toyota HiAce Super Long (H200)",
    "van", "GLOBAL", AD + "toyota-hiace-v-h200-facelift-2026-van-super-long-2.8l-151hp-super-ect-56596", 5.380, 1.880,
    2.285, 3.110, tf=1.655, tr=1.650, tc=12.2, kind="body", note="Common van/minibus in Asia, Africa, Australia.")
add("van_mercedes_esprinter_standard", "Large van Mercedes-Benz eSprinter standard (W907)",
    "Grote bestelbus Mercedes-Benz eSprinter standaard (W907)", "van", "manufacturer", "EU",
    AD + "mercedes-benz-esprinter-panel-van-w907-w910-55-kwh-116hp-43664", 2.687, 15.7 / 2,
    [unit("body", 6.088, 2.020, [axle(1.021, 1.759, False, True), axle(1.021 + 3.924, 1.770)])],
    ["track"],
    "Sprinter 3rd gen standard length (L2, WB 3924): 1021 + 3924 + 1152 mm (published sum 6097 vs length 6088). "
    "Track taken from the Sprinter Compact on the same platform (1759/1770 mm). Turning radius = half of the "
    "15.7 m turning circle.")
car("van_renault_master_l3h2", "Large van Renault Master L3H2", "Grote bestelbus Renault Master L3H2", "van", "EU",
    AD + "renault-master-iii-phase-iii-2019-panel-van-33-kwh-76hp-l3h2-direct-drive-40117", 6.225, 2.070, 2.500, 4.332,
    FO=0.869, tf=1.750, tr=1.730, tc=16.2, kind="body", note="L3H2 panel van (front-wheel drive); rear overhang 1.024 m.")
car("van_iveco_daily_l4h3", "Large van Iveco Daily L4H3", "Grote bestelbus Iveco Daily L4H3", "van", "EU",
    AD + "iveco-daily-iii-facelift-2021-panel-van-2.3-156hp-l4h3-single-wheel-57452", 7.283, 2.052, 2.848, 4.100,
    FO=1.057, tf=1.740, tr=1.704, tc=13.8, kind="body",
    note="Largest common panel van (single rear wheels); also a typical ambulance/minibus base. Rear overhang 2.126 m.")

# Two-wheelers (Wikipedia infobox data)
car("mc_bmw_r1250gs", "Motorcycle BMW R 1250 GS", "Motorfiets BMW R 1250 GS", "two_wheeler", "GLOBAL",
    "https://en.wikipedia.org/wiki/BMW_R1250GS", 2.207, 0.895, 1.45, 1.50, tf=0.0, tr=0.0, kind="bike",
    note="Large adventure motorcycle: L 2207, W 895 (handlebar), H 1450, WB 1500 mm.")
add("mc_yamaha_mt07", "Motorcycle Yamaha MT-07", "Motorfiets Yamaha MT-07", "two_wheeler", "manufacturer", "GLOBAL",
    "https://en.wikipedia.org/wiki/Yamaha_MT-07", None, None,
    [unit("bike", 2.085, 0.745, [axle(0.3425, 0.0, False, True), axle(0.3425 + 1.40, 0.0)])], ["front_overhang"],
    "Mid-size naked motorcycle: L 2085, W 745, WB 1400 mm, seat height 805 mm (overall height not given). "
    "Overhang split estimated (equal).")
car("scooter_honda_pcx125", "Scooter Honda PCX125", "Scooter Honda PCX125", "two_wheeler", "GLOBAL",
    "https://en.wikipedia.org/wiki/Honda_PCX", 1.915, 0.740, 1.090, 1.305, tf=0.0, tr=0.0, kind="bike",
    note="Best-selling 125 cc scooter: L 1915, W 740, H 1090, WB 1305 mm.")

# Construction (Caterpillar, archived product pages)
add("construction_excavator_cat_320", "Hydraulic excavator Caterpillar 320 (22 t)",
    "Rupsgraafmachine Caterpillar 320 (22 t)", "construction", "manufacturer", "GLOBAL",
    "http://web.archive.org/web/20240915102458/https://www.cat.com/en_US/products/new/equipment/excavators/"
    "medium-excavators/1000032602.html", 2.96, None,
    [unit("body", 9.53, 2.98, [axle(6.70 - 1.825, 2.38), axle(6.70 + 1.825, 2.38)])],
    ["axles"],
    "Shipping length 9530 (boom/stick folded forward), shipping height to top of cab 2960, transport width 2980, "
    "track gauge 2380, track length 4450, length to centre of rollers 3650, tail swing radius 2830 mm. 'Axles' "
    "are the idler/sprocket centres (3.65 m apart); the slew centre is assumed at mid-track, 2.83 m ahead of the "
    "counterweight, so positions are derived. Tracked: turns on the spot; tail swing radius 2.83 m.")
add("construction_wheel_loader_cat_950", "Wheel loader Caterpillar 950", "Wiellader Caterpillar 950", "construction",
    "manufacturer", "GLOBAL",
    "http://web.archive.org/web/20250322050642/https://www.cat.com/en_US/products/new/equipment/wheel-loaders/"
    "medium-wheel-loaders/124880.html", 3.456, None,
    [unit("body", 7.024, 2.824, [axle(7.024 - 2.055 - 3.35, 2.14), axle(7.024 - 2.055, 2.14)])],
    ["front_overhang"],
    "CAT 950 standard lift: overall length without bucket 7024, wheelbase 3350, width over tyres 2824, tread "
    "2140, height to top of ROPS 3456 mm. Articulated steering (hitch at mid-wheelbase), so neither axle steers. "
    "Rear axle to counterweight 2055 mm taken from the CAT 950 GC page (estimate) to place the axles.")

# ---------------------------------------------------------------------------
json.dump(V, open(OUT, "w"), indent=1, ensure_ascii=False)
print(len(V), "vehicles written to", OUT)
