% vim: filetype=prolog
:- module(basic). 
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
drawing_object(box).
