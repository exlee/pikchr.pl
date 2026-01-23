% vim: filetype=prolog

semicolon --> ";".
space --> " ".
nl --> "\n".
quote --> "\"".

quoted(Name)  --> "\"", Name, "\"".
squared(Expr) --> "[", Expr, "]".

group(Label, Expr) --> { atom(Label) }, label(Label), ": ", squared(Expr), nl.
group(Label, Expr) --> { string(Label), string_upper(Label, Up) }, Up, ": ", squared(Expr), nl.
group(label(Label), Expr) --> label(Label), ": ", squared(Expr), nl.

group_start(Label) --> label(Label), ": [".
group_end --> "]", nl.

expr(V) --> V, nl.

lines(A,B,C) --> quoted(A), space, quoted(B), space, quoted(C).
lines(A,B) --> quoted(A), space, quoted(B).
lines(A) --> quoted(A).

label(label(L)) --> label(L).
label(I) -->  {
   ( atomic(I)
   -> format(string(S),"~w", [I])
   ;  format(string(S),"~s", [I])
   ),  replace(S, '.', '_', NS), string_upper(NS, U) },  U.

label(I) -->  {  \+atomic(I), string_upper(I, U) },  U.

as(Atom) --> format_("~w", [Atom]).
ase(Atom) --> expr(as(Atom)).
asq(Atom) --> quoted(as(Atom)).
at(Comp) --> { Comp =.. [K, Value] }, as(K), space, as(Value).
at(K, V) --> as(K), space, as(V).
int(V) --> format("~d", [V]).

space_separated([Item]) --> Item.
space_separated([H|Rest]) --> H, space, space_separated(Rest).
space_separated([]) --> [].


