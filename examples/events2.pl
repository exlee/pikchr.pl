move_p(P) --> "move ", format_("~w%", [P]), ";".

pad(Amount, Shape) --> move_p(Amount), Shape, move_p(Amount).

event_shape(ID) --> pad(30,
    format_("circle \"~w\" height 50%;", [ID])
  ).
label(Atom) -->
  {
    atom_chars(Atom, Chars),
    string_upper(Chars, Label)
  }, Label.
  
event(ID) --> label(ID), ": [", event_shape(ID), "];".

rollback_shape(ID,To) -->
  label(ID), ": [",
  format_("box \"rollback (~w)\" height 50%",[To]), "];".

rollback(ID,To) --> pad(10, rollback_shape(ID,To)).

anchor(u) --> ".n".
anchor(d) --> ".s".
dir(d) --> "down".
dir(u) --> "up".
dir(r) --> "right".
dir(l) --> "left".
num(V) --> format_("~d", [V]).
link(DirY, DirX, Pad, From, To) -->
  "spline from ", label(From), anchor(DirY),
  " then ", dir(DirY), " ", num(Pad), "%",
  " then ", dir(DirX), " until even with ", label(To),
  " then to ", label(To), " chop", " ->", ";".

diagram -->
  event(e1),
  event(e2),
  event(e3),
  rollback(r1,e3),
  event(e4), 
  link(u,r, 30, e1,e2),
  link(u,r, 30, e2,e3),
  link(u,r, 30, e3,r1),
  link(d,l, 30, r1,e2),
  link(d,r, 80, e2,e4).