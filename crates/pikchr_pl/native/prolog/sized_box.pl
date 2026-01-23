% vim: filetype=prolog
:- module(sized_box).

sized_box(L, W,H) --> sized_box(L,W,H,[]). 
sized_box(L, W,H,Attrs) -->
  group(L,
    basic:box(' ', (format_("width ~d% height ~d%", [W,H]), space, space_separated(Attrs)))
  ), nl.


