"""Headless check of the city editor: draw a small city by clicking, export, validate.

Usage (from the repository root, with Playwright for Python installed):
    python3 -m http.server 8123 &
    python3 editor/test_editor.py http://localhost:8123/editor/index.html out_dir
Then:  citysim check-map out_dir/map.json
"""
import json, os, sys
from playwright.sync_api import sync_playwright

url = sys.argv[1]
out = sys.argv[2]
os.makedirs(out, exist_ok=True)

with sync_playwright() as p:
    b = p.chromium.launch()
    pg = b.new_page(viewport={"width": 1400, "height": 800})
    errors = []
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.goto(url)
    pg.evaluate("window.__editor.load({name:'x',nodes:[],edges:[]},'blank')")
    # Zoom so that 10 m = 20 px and the origin is near the left of the canvas.
    box = pg.locator("#cv").bounding_box()

    def screen(x, y):
        s = pg.evaluate(f"window.__editor.toScreen({{x:{x},y:{y}}})")
        return box["x"] + s["x"], box["y"] + s["y"]

    def click(x, y):
        sx, sy = screen(x, y)
        pg.mouse.click(sx, sy)

    pg.evaluate("document.getElementById('bFit').click()")
    pg.keyboard.press("r")
    # Fit view on an empty map gives scale 2 around (150, 0).
    for pt in [(0, 0), (150, 0), (300, 0)]:
        click(*pt)
    pg.keyboard.press("Escape")
    for pt in [(150, -100), (150, 0), (150, 100)]:   # crossing road through the middle node
        click(*pt)
    pg.keyboard.press("Escape")
    n_roads = len([e for e in pg.evaluate("window.__editor.map.edges") if e["kind"] == "road"])
    assert n_roads == 4, n_roads

    pg.keyboard.press("c")
    click(60, 0)       # crossing on the west main-road segment
    click(150, -50)    # crossing on the side road
    m = pg.evaluate("window.__editor.map")
    crossings = [e for e in m["edges"] if e["kind"] == "crossing"]
    assert len(crossings) == 2, crossings

    pg.keyboard.press("l")
    click(150, 0)
    assert len(pg.evaluate("window.__editor.map.trafficLights")) == 1

    # Footpaths along the north side of the main road, through the crossing end nodes.
    m = pg.evaluate("window.__editor.map")
    nodes = {n["id"]: n["pos"] for n in m["nodes"]}
    c0 = crossings[0]
    north = max((nodes[c0["from"]], nodes[c0["to"]]), key=lambda q: q["y"])
    pg.keyboard.press("f")
    for pt in [(0, 8), (north["x"], north["y"]), (130, 8)]:
        click(*pt)
    pg.keyboard.press("Escape")

    pg.select_option("#optKind", "home")
    pg.keyboard.press("b")
    sx0, sy0 = screen(20, 20); sx1, sy1 = screen(100, 60)
    pg.mouse.move(sx0, sy0); pg.mouse.down(); pg.mouse.move(sx1, sy1); pg.mouse.up()
    pg.select_option("#optKind", "office")
    sx0, sy0 = screen(200, 20); sx1, sy1 = screen(280, 60)
    pg.mouse.move(sx0, sy0); pg.mouse.down(); pg.mouse.move(sx1, sy1); pg.mouse.up()

    pg.keyboard.press("Control+z"); pg.keyboard.press("Control+y")   # undo/redo round trip
    m = pg.evaluate("window.__editor.map")
    assert len(m["buildings"]) == 2, len(m["buildings"])

    v = pg.evaluate("window.__editor.validate()")
    print("errors:", v["errs"]); print("warnings:", v["warns"])
    exported = pg.evaluate("window.__editor.exportMap()")
    with open(os.path.join(out, "map.json"), "w") as f:
        f.write(exported + "\n")
    pg.screenshot(path=os.path.join(out, "editor.png"))
    # Loading the example city must work too.
    pg.select_option("#bExample", "../cities/tiny-town/map.json")
    pg.wait_for_timeout(500)
    assert len(pg.evaluate("window.__editor.map.nodes")) == 36
    pg.screenshot(path=os.path.join(out, "editor-tiny-town.png"))
    assert not errors, errors
    b.close()
print("editor test ok")
