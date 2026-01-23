% vim: filetype=prolog

:- module(grid).
grid2x2(A,B,C,D) -->
  label("GRID",
    (box(A), "down", box(C), "right", box(D), "up", box(B))).
