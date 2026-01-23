% vim: filetype=prolog

:- module(txt).
text_above(L, T) --> "move to ", L, ".nw;", "text ", quoted(T), " center above ;". 
text_inside(L, T) --> "text ", quoted(T), "center with .center at ", L, ".center".

text_inner(At, Text) --> "dot invis;", "text ", asq(Text), "below  at ", label(At), ".n", ";move to last dot;", nl.
text_outer(At, Text) --> "text ", asq(Text), "above  at ", label(At), ".n + (0,0.05)", nl,
                         "dot at ", label(At), ".s + (0,-0.20) invis", nl.

text_center(At, Text) -->
  "dot invis;",
  "text ", asq(Text), "at ", label(At), nl.

