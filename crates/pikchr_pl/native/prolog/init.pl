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
drawing_object(arc).
drawing_object(arrow).
drawing_object(box).
drawing_object(circle).
drawing_object(cylinder).
drawing_object(diamond).
drawing_object(dot).
drawing_object(ellipse).
drawing_object(file).
drawing_object(line).
drawing_object(move).
drawing_object(oval).
drawing_object(spline).
drawing_object(text).

basic_term(chop).
basic_term(fit).
basic_term(ccw).
basic_term(cw).
% Text Attribute
basic_term(above).
basic_term(aligned).
basic_term(below).
basic_term(big).
basic_term(bold).
basic_term(mono).
basic_term(monospace).
basic_term(center).
basic_term(italic).
basic_term(ljust).
basic_term(rjust).
basic_term(small).

% Misc property
attr_term(fill).
attr_term(color).
attr_term(behind).
% Numeric Property
attr_term(diameter).
attr_term(ht).
attr_term(height).
attr_term(rad).
attr_term(radius).
attr_term(thickness).
attr_term(width).
attr_term(wid).

semicolon --> ";".
space --> " ".
nl --> "\n".

quoted(Name)  --> "\"", Name, "\"".
squared(Expr) --> "[", Expr, "]".
label(Label, Expr) --> Label, ": ", squared(Expr), semicolon.

lines(A,B,C) --> quoted(A), space, quoted(B), space, quoted(C).
lines(A,B) --> quoted(A), space, quoted(B).
lines(A) --> quoted(A).

exprs_([H]) --> H, nl.
exprs_([H|T]) --> H, nl, exprs_(T).

words_([H]) --> H.
words_([H|T]) --> H, space, words_(T).

attrs(Any) --> words(Any).

expr(V1) --> V1, nl.

exprs(V1) --> expr(V1).
exprs(V1,V2) --> exprs_([V1,V2]).
exprs(V1,V2,V3) --> exprs_([V1,V2,V3]).
exprs(V1,V2,V3,V4) --> exprs_([V1,V2,V3,V4]).
exprs(V1,V2,V3,V4,V5) --> exprs_([V1,V2,V3,V4,V5]).
exprs(V1,V2,V3,V4,V5,V6) --> exprs_([V1,V2,V3,V4,V5,V6]).
exprs(V1,V2,V3,V4,V5,V6,V7) --> exprs_([V1,V2,V3,V4,V5,V6,V7]).
exprs(V1,V2,V3,V4,V5,V6,V7,V8) --> exprs_([V1,V2,V3,V4,V5,V6,V7,V8]).
exprs(V1,V2,V3,V4,V5,V6,V7,V8,V9) --> exprs_([V1,V2,V3,V4,V5,V6,V7,V8,V9]).
exprs(V1,V2,V3,V4,V5,V6,V7,V8,V9,V10) --> exprs_([V1,V2,V3,V4,V5,V6,V7,V8,V9,V10]).

words(V1) --> words_([V1]).
words(V1,V2) --> words_([V1,V2]).
words(V1,V2,V3) --> words_([V1,V2,V3]).
words(V1,V2,V3,V4) --> words_([V1,V2,V3,V4]).
words(V1,V2,V3,V4,V5) --> words_([V1,V2,V3,V4,V5]).
words(V1,V2,V3,V4,V5,V6) --> words_([V1,V2,V3,V4,V5,V6]).
words(V1,V2,V3,V4,V5,V6,V7) --> words_([V1,V2,V3,V4,V5,V6,V7]).
words(V1,V2,V3,V4,V5,V6,V7,V8) --> words_([V1,V2,V3,V4,V5,V6,V7,V8]).
words(V1,V2,V3,V4,V5,V6,V7,V8,V9) --> words_([V1,V2,V3,V4,V5,V6,V7,V8,V9]).
words(V1,V2,V3,V4,V5,V6,V7,V8,V9,V10) --> words_([V1,V2,V3,V4,V5,V6,V7,V8,V9,V10]).

grid2x2(A,B,C,D) -->
  label("GRID",
  exprs(box(A), "down", box(C), "right", box(D), "up", box(B))).



fas__file_text(N,'') --> fas__file_text(N, ' ').
fas__file_text(N, T) --> 
  { replace(T, '"', '\\"', NT) },
  format_("L~d: text \"~d: ~s\" ljust~n", [N, N, NT]).

fas__file_lines(_,[]) --> [].
fas__file_lines(N,[H|T]) -->
  fas__file_text(N,H), { NN is N + 1 },
  fas__file_lines(NN,T).

            
file_as_lines(File) --> 
  "down; ",
  { getfile(File, Lines) },
  fas__file_lines(1, Lines).
