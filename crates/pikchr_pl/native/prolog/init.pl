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
			expr(words(NameChars, quoted(Label)))
    ).
generate_drawing_rule(Name, (Head --> Body)) :-
		atom_chars(Name, NameChars),
    Head =.. [Name, Label,Attrs],
    Body = (
			expr(words(NameChars, quoted(Label), Attrs))
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
group(Label, Expr) --> Label, ": ", expr(squared(Expr)).
expr(V) --> V, nl.

lines(A,B,C) --> quoted(A), space, quoted(B), space, quoted(C).
lines(A,B) --> quoted(A), space, quoted(B).
lines(A) --> quoted(A).

label(I) -->  {  format(string(S), "~w", [I]), string_upper(S, U) },  U.

as(Atom) --> format_("~w", [Atom]).
ase(Atom) --> expr(as(Atom)).
asq(Atom) --> quoted(as(Atom)).
int(V) --> format("~d", [V]).

mod(grid).
grid2x2(A,B,C,D) -->
  label("GRID",
    (box(A), "down", box(C), "right", box(D), "up", box(B))).

mod(fas).
file_text(N,'') --> file_text(N, ' ').
file_text(N, T) --> 
  { replace(T, '"', '\\"', NT) },
  format_("L~d: text \"~d: ~s\" ljust~n", [N, N, NT]).

file_lines(_,[]) --> [].
file_lines(N,[H|T]) -->
  file_text(N,H), { NN is N + 1 },
  file_lines(NN,T).

            
file_as_lines(File) --> 
  "down; ",
  { getfile(File, Lines) },
  file_lines(1, Lines).

mod(place_text).
text_above(L, T) --> "move to ", L, ".nw;", "text ", quoted(T), " center above ;". 
text_inside(L, T) --> "text ", quoted(T), "center with .center at ", L, ".center".

mod(sized_box).
sized_box(L, W,H) --> sized_box(L,W,H, [""]). 
sized_box(L, W,H,[]) --> sized_box(L,W,H, [""]). 
sized_box(L, W,H,Attrs) -->
  { phrase(words(Attrs), Out) },
  group(L,
    box("", (format_("width ~d% height ~d% ~s", [W,H,Out])))), nl.

