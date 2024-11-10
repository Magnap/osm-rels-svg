# OpenStreetMap relation/way SVG export
This tool allows you to create an SVG of a set of OSM relations and ways.
It uses the WGS 84 Web Mercator projection (EPSG:3857).

Relations are input as text files with an OSM id per line.
The tool will then parse an `.osm.pbf` file,
and will create an SVG that contains
each relation as a group,
recursing until it can create a path per way.
Extra ways can be added through a separate text file.