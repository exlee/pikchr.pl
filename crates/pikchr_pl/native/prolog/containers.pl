% vim: filetype=prolog
:- module(containers).
offset_sign(nw, "-", "").
offset_sign(se, "", "-").
offset_sign(sw, "-", "-").
offset_sign(ne, "", "").
offset(Dir, OfX, OfY) --> { offset_sign(Dir, X, Y) }, format_("(~s~2f, ~s~2f)", [X,OfX,Y,OfY]).
%% around(Label, OffsetX, OffsetY, LabelToSurround).
%%%
%% surrounds group (or object) specified by label with a line
around(Label, OfX, OfY, What) -->
  label(Label), ": ", 
  "line from ", label(What), ".nw + ", offset(nw,OfX,OfY),
  "then to ", label(What), ".ne +", offset(ne, OfX, OfY), 
  "then to ", label(What), ".se +", offset(se, OfX, OfY),
  "then to ", label(What), ".sw +", offset(sw, OfX, OfY),
  "then close",
  nl.
