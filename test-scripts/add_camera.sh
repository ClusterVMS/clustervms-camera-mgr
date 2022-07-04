#!/bin/sh
# usage: ./add_camera.sh '{"id":42, "name":"KittyCamera"}'
curl -X POST http://localhost:8000/v0/cameras/ -H 'Content-Type: application/json' -d $1
