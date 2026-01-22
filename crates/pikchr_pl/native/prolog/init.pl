% vim: filetype=prolog
:- use_module(library(format)).
:- use_module(library(dcgs)).


term_expansion(drawing_object(Name), PrologClauses) :-
    findall(Rule, generate_drawing_rule(Name, Rule), Rules), 
    maplist(expand_term, Rules, PrologClauses)
.
term_expansion(basic_term(Name), PrologClauses) :-
    findall(Rule, generate_basic_term(Name, Rule), Rules), 
    maplist(expand_term, Rules, PrologClauses)
.
term_expansion(attr_term(Name), PrologClauses) :-
    findall(Rule, generate_attr_term(Name, Rule), Rules), 
    maplist(expand_term, Rules, PrologClauses)
.

generate_attr_term(Name, (Head --> Body)) :-
	atom_chars(Name, NameChars), 
	Head =.. [Name, Value],
	Body = words(NameChars, Value).

generate_basic_term(Name, (Name --> Body)) :-
	atom_chars(Name, NameChars), 
	Body = NameChars.

%% Drawing Rules for Objects
generate_drawing_rule(Name, (Head --> Body)) :-
		atom_chars(Name, NameChars),
    Head =.. [Name],
    Body = (
			expr(NameChars)	
    ).
generate_drawing_rule(Name, (Head --> Body)) :-
		atom_chars(Name, NameChars),
    Head =.. [Name, Label],
    Body = (
     
      as(Name), asq(Label), nl
    ).
generate_drawing_rule(Name, (Head --> Body)) :-
		atom_chars(Name, NameChars),
    Head =.. [Name, Label,Attrs],
    Body = (
      as(Name), asq(Label), space, Attrs, nl
    ).
%% Drawing Rules For Objects
%drawing_object(arc).
%drawing_object(arrow).
%drawing_object(box).
%drawing_object(circle).
%drawing_object(cylinder).
%drawing_object(diamond).
%drawing_object(dot).
%drawing_object(ellipse).
%drawing_object(file).
%drawing_object(line).
%drawing_object(move).
%drawing_object(oval).
%drawing_object(spline).
%drawing_object(text).
%
%basic_term(chop).
%basic_term(fit).
%basic_term(ccw).
%basic_term(cw).
%% Text Attribute
%basic_term(above).
%basic_term(aligned).
%basic_term(below).
%basic_term(big).
%basic_term(bold).
%basic_term(mono).
%basic_term(monospace).
%basic_term(center).
%basic_term(italic).
%basic_term(ljust).
%basic_term(rjust).
%basic_term(small).
%
%% Misc property
%attr_term(fill).
%attr_term(color).
%attr_term(behind).
%% Numeric Property
%attr_term(diameter).
%attr_term(ht).
%attr_term(height).
%attr_term(rad).
%attr_term(radius).
%attr_term(thickness).
%attr_term(width).
%attr_term(wid).

semicolon --> ";".
space --> " ".
nl --> "\n".
quote --> "\"".


quoted(Name)  --> "\"", Name, "\"".
squared(Expr) --> "[", Expr, "]".

group(Label, Expr) --> { atom(Label) }, label(Label), ": ", squared(Expr), nl.
group(Label, Expr) --> { string(Label), string_upper(Label, Up) }, Up, ": ", squared(Expr), nl.
group(label(Label), Expr) --> label(Label), ": ", squared(Expr), nl.

expr(V) --> V, nl.

lines(A,B,C) --> quoted(A), space, quoted(B), space, quoted(C).
lines(A,B) --> quoted(A), space, quoted(B).
lines(A) --> quoted(A).

label(I) -->  {
   ( atomic(I)
   -> format(string(S), "~w", [I])
   ;  format(string(S), "~s", [I])
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

:- module(grid).
grid2x2(A,B,C,D) -->
  label("GRID",
    (box(A), "down", box(C), "right", box(D), "up", box(B))).

:- module(file_as_lines).

file_text(N,'') --> file_text(N, ' ').
file_text(N, T) --> 
  { replace(T, '"', '\\"', NT) },
  format_("L~d: text \"~d: ~s\" ljust~n", [N, N, NT]).

file_lines(_,[]) --> [].
file_lines(N,[H|T]) -->
  file_text(N,H), { NN is N + 1 },
  file_lines(NN,T).

            
show(File) --> 
  "down; ",
  { getfile(File, Lines) },
  file_lines(1, Lines).

:- module(txt).
text_above(L, T) --> "move to ", L, ".nw;", "text ", quoted(T), " center above ;". 
text_inside(L, T) --> "text ", quoted(T), "center with .center at ", L, ".center".

text_inner(At, Text) --> "dot invis;", "text ", asq(Text), "below  at ", label(At), ".n", ";move to last dot;", nl.
text_outer(At, Text) --> "text ", asq(Text), "above  at ", label(At), ".n + (0,0.05)", nl.

text_center(At, Text) -->
  "dot invis;",
  "text ", asq(Text), "at ", label(At), nl.

:- module(basic). 
drawing_object(box).

:- module(sized_box).

sized_box(L, W,H) --> sized_box(L,W,H,[]). 
sized_box(L, W,H,Attrs) -->
  group(L,
    basic:box(' ', (format_("width ~d% height ~d%", [W,H]), space, space_separated(Attrs)))
  ), nl.


:- module(shapes).

socket(L, Dir, T) --> "box at ", label(L), ".", as(Dir),
                      " width 20%", " fit", " fill white", semicolon,
                      "line from last box.s to last box.n invisible ", quoted(T), " aligned",
                      nl.
entry(L, T) --> socket(L,w,T).
exit(L, T) --> socket(L,e,T).
box(L) --> label(L), ": ", as(box), space, as(rad), " 0.05 ".
move_to(L, Dir) --> "move to ", label(L), ".", as(Dir), nl. 
pipe(L, A, Entry, Exit) --> box(L), " height 80%", " width 80%",
               semicolon, place_text:text_outer(L, A),
               entry(L, Entry),
               exit(L, Exit),
               move_to(L, e).

:- module(testing).
:- dynamic(test/1).
test(label_atom) :- phrase(label(a), "A"). 
test(label_string) :- phrase(label("abc"), "ABC").
test(label_replace) :- phrase(label('hello.pl'), "HELLO_PL").
test(attr) :- phrase(at(fill, red), "fill red").
test(attr_compound) :- phrase(at(fill(red)), "fill red").
test(group_string) :- phrase(group("abc", ase(box)), "ABC: [box\n]\n").
test(group_atom) :- phrase(group(def, ase(box)), "DEF: [box\n]\n").
test(group_label) :- phrase(group(label(ghi), ase(box)),  "GHI: [box\n]\n").

test_result(X) :-
  ( test(X)
  -> R = ok
  ;  R = failure
  ),
  format("~2+..~a~20|~t ~a~n", [X,R]).
tests :-
    format("~ntesting~n~n"),
    forall(clause(test(Name), _Body), test_result(Name)),
    nl.


%diagram --> shapes:pipe("A","B","C","D").

:- module(runtime).
run :- phrase(query:diagram, Out), format("~s", [Out]).
:- module(query).

