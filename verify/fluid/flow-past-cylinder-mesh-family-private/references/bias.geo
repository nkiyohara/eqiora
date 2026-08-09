// Eqiora-authored alternate-provider structural MESH0 fixture recipe.
// It realizes the same finest chordal Geometry with a different meshing recipe.
SetFactory("Built-in");

segments = DefineNumber[32, Name "Parameters/segments"];
mesh_size = DefineNumber[0.04, Name "Parameters/mesh_size"];

x_min = 0.0;
x_max = 2.2;
y_min = 0.0;
y_max = 0.41;
cx = 0.2;
cy = 0.2;
radius = 0.05;

corner_x[] = {x_max, x_min, x_min, x_max};
corner_y[] = {y_max, y_max, y_min, y_min};
corner_angle[] = {
  Atan2(y_max - cy, x_max - cx),
  Atan2(y_max - cy, x_min - cx),
  2 * Pi + Atan2(y_min - cy, x_min - cx),
  2 * Pi + Atan2(y_min - cy, x_max - cx)
};

outer_ray[] = {};
For i In {0:segments - 1}
  theta = 2 * Pi * i / segments;
  dx = Cos(theta);
  dy = Sin(theta);
  tx = 1.e300;
  ty = 1.e300;
  If (dx > 0)
    tx = (x_max - cx) / dx;
  ElseIf (dx < 0)
    tx = (x_min - cx) / dx;
  EndIf
  If (dy > 0)
    ty = (y_max - cy) / dy;
  ElseIf (dy < 0)
    ty = (y_min - cy) / dy;
  EndIf
  travel = Min(tx, ty);
  outer_ray[i] = newp;
  Point(outer_ray[i]) = {cx + travel * dx, cy + travel * dy, 0, mesh_size};
EndFor

outer_path[] = {};
For i In {0:segments - 1}
  outer_path[] += {outer_ray[i]};
  low = 2 * Pi * i / segments;
  high = 2 * Pi * (i + 1) / segments;
  For j In {0:3}
    If (corner_angle[j] > low && corner_angle[j] < high)
      corner_point = newp;
      Point(corner_point) = {corner_x[j], corner_y[j], 0, mesh_size};
      outer_path[] += {corner_point};
    EndIf
  EndFor
EndFor

outer_lines[] = {};
For i In {0:#outer_path[] - 1}
  outer_lines[i] = newl;
  Line(outer_lines[i]) = {outer_path[i], outer_path[(i + 1) % #outer_path[]]};
EndFor

circle[] = {};
For i In {0:segments - 1}
  theta = 2 * Pi * i / segments;
  circle[i] = newp;
  Point(circle[i]) = {cx + radius * Cos(theta), cy + radius * Sin(theta), 0, mesh_size};
EndFor

circle_lines[] = {};
For i In {0:segments - 1}
  circle_lines[i] = newl;
  Line(circle_lines[i]) = {circle[i], circle[(i + 1) % segments]};
EndFor

outer_loop = newll; Curve Loop(outer_loop) = {outer_lines[]};
hole_loop = newll; Curve Loop(hole_loop) = {circle_lines[]};
fluid = news; Plane Surface(fluid) = {outer_loop, hole_loop};

Mesh.Algorithm = 6;
Mesh.ElementOrder = 1;
Mesh.SaveAll = 1;
Mesh.MshFileVersion = 4.1;
Mesh.Binary = 0;
Mesh.RandomFactor = 0;
