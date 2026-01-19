sc --> ";".

move_p(P) --> "move ", format_("~w%", [P]), ";".

pad(Amount, Shape) -->
  move_p(Amount),
  Shape,
  move_p(Amount).

event_shape(ID) -->
  pad(30,
    format_("circle \"~w\" height 50%;", [ID])
  ).
label(Atom) -->
  {
    atom_chars(Atom, Chars),
    string_upper(Chars, Label)
  }, Label.
  
event(ID) --> 
  label(ID), ": [", event_shape(ID), "];".

rollback_shape(ID,To) -->
  label(ID), ": [",
  format_("box \"rollback (~w)\" height 50%",[To]), "];".
rollback(ID,To) -->
  pad(10, rollback_shape(ID,To)).

link(u,r, E1, E2) -->
  "arc from ", label(E1), ".n to ", label(E2), ".n -> cw;".
link(d,l, E1, E2) -->
  "arc from ", label(E1), ".s to ", label(E2), ".s -> cw;".
link(d,r, E1, E2) -->
  "arc from ", label(E1), ".s to ", label(E2), ".s -> ccw;".
diagram -->
  event(e1),
  event(e2),
  event(e3),
  rollback(r1,e3),
  event(e4), 
  link(u,r, e1,e2),
  link(u,r, e2,e3),
  link(u,r, e3,r1),
  link(d,l, r1,e2),
  link(d,r, e2,e4).