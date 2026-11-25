"""What Settings stores for each control, and how the gates read the config file back.

The stored value of a row comes from rows.json (the answers file's key list). `value_for` and `option_for` are the
two directions of a segmented row (a row's `values` list, where it has one, names what each option stores);
`coerce` turns the text of a field into the stored value; `read_config` reads the config file the window writes,
JSON or the XML of today's config.xml (one element per setting), with a parser of its own.
"""

import json
from pathlib import Path
from xml.etree import ElementTree

ROWS = json.loads(
    (Path(__file__).resolve().parent / "rows.json").read_text(encoding="utf-8")
)["rows"]


def row_for(label):
    return next(r for r in ROWS if r["label"] == label)


def value_for(row, option):
    """The value a chosen segment option stores."""
    options = row["options"]
    if "values" in row:
        return row["values"][options.index(option)]
    if row["key"] == "theme":
        return "system" if option == "On" else "dark"
    if row["key"] == "new_window_panes":
        return options.index(option) + 1
    if isinstance(row["initial"], bool):
        return option == options[0]
    return option.lower()


def option_for(row, value):
    """The segment option a stored value stands for."""
    options = row["options"]
    if "values" in row:
        return options[row["values"].index(value)]
    if row["key"] == "theme":
        return "On" if value == "system" else "Off"
    if row["key"] == "new_window_panes":
        return options[int(value) - 1]
    if isinstance(row["initial"], bool):
        return options[0] if value else options[1]
    return next(o for o in options if o.lower() == str(value))


def coerce(row, text):
    """The stored value of a field's text: a number where the setting's initial value is one."""
    initial = row["initial"]
    if isinstance(initial, bool) or not isinstance(initial, (int, float)):
        return text
    number = float(text)
    return int(number) if isinstance(initial, int) and number == int(number) else number


def display_text(value):
    """The text a field shows for a stored value: a whole number without a trailing ".0"."""
    if isinstance(value, float) and value == int(value):
        return str(int(value))
    return str(value)


def parse_scalar(text):
    if text in ("true", "false"):
        return text == "true"
    for kind in (int, float):
        try:
            return kind(text)
        except ValueError:
            continue
    return text


def read_config(path):
    """The config file's settings as {key: value}; scalars only (the structured settings are not in rows.json)."""
    path = Path(path)
    if path.suffix == ".json":
        return json.loads(path.read_text(encoding="utf-8"))
    root = ElementTree.fromstring(path.read_text(encoding="utf-8"))
    return {
        child.tag: parse_scalar((child.text or "").strip())
        for child in root
        if len(child) == 0
    }


def write_config(path, values):
    Path(path).write_text(json.dumps(values, indent=1) + "\n", encoding="utf-8")


def initial_config():
    return {row["key"]: row["initial"] for row in ROWS}
