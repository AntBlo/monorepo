;FLAVOR:Marlin
;Layer height: 0.2
M190 S0 ;Bed Temp Temp -- Change this
M104 S0 ;Extruder Temp -- Set to 0 for initial calibration
M109 S ;Extruder Temp -- Set to 0 for initial calibration
M82 ;absolute extrusion mode

;Basic settings:
;Layer height: 0.2
;Walls: 0.8
;Top/Bottom: 0.6
;Fill: 10
;Retraction Distance: 7
;Retraction Speed: 60
G28 ;Home

; Calibration Round 1
G1 Z10 ; Lift Z axis
G1 X32 Y35 F2400; Move to Position 1
G1 Z0
G1 Z10 ; Lift Z axis
G1 X32 Y206 F2400; Move to Position 2
G1 Z0
G1 Z10 ; Lift Z axis
G1 X202 Y206 F2400 ; Move to Position 3
G1 Z0
G1 Z10 ; Lift Z axis
G1 X202 Y35 F2400 ; Move to Position 4
G1 Z0

G28 ;Home
M106 S0 ; turn off cooling fan
M104 S0 ; turn off extruder
M140 S0 ; turn off bed
M84 ; disable motors
M82 ;absolute extrusion mode
M104 S0
;End of Gcode
