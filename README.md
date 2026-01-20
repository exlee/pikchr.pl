# pikchr.pl (Pikchr in Prolog)

`pikchr.pl` is composed of three things:
- **pikchr_pro** - a simple transformer from the Prolog Code to SVG
- **pikchr_pl** - a GUI application written in Iced 
- Plug-and-play library for transforming diagrams into SVGs (part of `pikchr_pro`) supporting both sync and async

## Plead 

This project is a fruit of many years of research on various diagrams solutions and I believe it reached endgame. If you think it's worth developing further please sponsor it with a $(VALUE_OF_A_COFFEE) monthly or some. Thanks in advance.

## Installation

Warning: build process is something I'm working on right now, it might fail for unknown reasons

```
cargo install --git https://github.com/exlee/pikchr.pl
```

or

```
git clone git@github.com:exlee/pikchr.pl.git
cd pikchr.pl
cargo install --path .
```
## Usage
### pikchr.pl (GUI)

`pikchr.pl` expects that `diagram//0` is specified (Prolog people: run/0 is pretty much `run :- phrase(diagram, Out), write(Out).` + initializers. 

#### Included Predicates

These are included by default, however are subject to change (personally I found not having them in diagram is actually easier than working around them).

Helper clauses are included in `pikchr.pl`, some of them:
- `expr//1` - adds newline at the end of Expression
- `quoted//1, squared//1` - wrapping in quotes and brackets respectively
- `lines//1, .., lines//3` - Pikchr take up to 3 strings for most labels, helper for that case
- `words//1 alias attrs//1` - separate items in provided list using space
- `<obj>//0, <obj>//1, <obj>//2` - drawing helpers for Pikchr objects (box,circle,cylinder,diamond,dot,file,ellipse,line, etc.), for example: `box, box("Name"), box("Name", ("fill red"))`
- `<single_word>//0` - single word helpers for many (but not all!) Pikchr words, e.g. `chop`, `small`, etc.
- `<attr>//1` - attribute helpers for single word values for some attributes of Pikchr, e.g. `fill("red")`, `color("blue")`.
- `exprs//2, .., exprs//10` and `words//2, .., words//10`  - convenience helpers e.g. `exprs(a,b,c,d) = exprs([a,b,c,d])`.
- `nl`, `space`, `semicolon` reusable elements

Following "smart objects" predicates are included
- `grid2x2//4`  - grid with 4 labels
- `files_as_lines//1` - renders file line-by-line, note that it's WASM escaped and application CWD is WASM filesystem root!

#### Caveat: Long Lines in pikchr.pl

Prefer `\n` over `;` when separating expressions in Pikchr.

Reason for that is that when using `;` instead of `\n` for expression separation errors can slow down `pikchr.pl` considerably. Pikchr shows where exactly error occurred and for 5000 long line it will show >15000 chars:
- Few lines of errors (and assumed 5000 characters long line)
- Space padded `^^^^` indicators (if error happens at the end of long line - another 5000 characters)
- Aligned error message (+5000 characters)

Rendering this long line takes considerate time. Some prevention measures are in place but experience deteriorates nonetheless.
### pikchr.pro (CLI)

`pikchr_pro` is both library for integration (I'm using it myself for Editor/Previewer) and a CLI utility. CLI utility is as simple as it gets - it reads Prolog file on STDIN and outputs transformed diagram through Pikchr into STDOUT.

```
cat my_diagram.pl | pikchr_pro > output.svg
```

The only requirement is usage of `diagram//0` DCG definition, as it is starting point for the wrapper. Note that no Pikchr utilities are included, so everything has to be provided pretty much from scratch through DCG.

This means that it's not possible to escape learning oneself some Prolog (thankfully DCGs are one of the easiest features) or [Pikchr].

### pikchr.pro (library)

See source code, not much documentation for now (sorry!).

There are two provided runners - sync and async ones. Since they're 90% same they're implemented as macros with `_impl!` suffix.

#### Caveat: Warmup

Initial loading of WASM binary takes approx. 1.5 seconds. Later on it responds fast, but first drawing is going always to be slow. It's possible to warm up preemptively by using `init()` function.

## Rationale / Architecture

[Pikchr] has been my favorite diagramming language for the long time and Prolog is my pet language for even longer. One day I was researching ways of creating diagrams declaratively and crazy idea popped in my head. What if I used Definite Clause Grammars (DCGs) and then used them to generate Pikchr code. 

Initially I made attempt to integrate [Scryer Prolog] however it failed. It seems that Scryer panics quite hard when it gets unexpected input etc, which made it impossible to get "live latency" through it (not to mention it took the whole app with itself).

Then I found out about [Trealla Prolog] and decided to run it through WASM (for Safety and Profit). Initial warmup takes some time (that's why CLI takes approx. 1.5 seconds to complete) but later calls are quick to execute (thus potential future live watcher could actually be fast). Outside of the fact that it integrates nicely it is also a one awesome of the Prolog implementation (love error messages). 

## About Pikchr and Prolog

### Pikchr

Just skim [Pikchr: Pikchr User Manual](https://pikchr.org/home/doc/trunk/doc/userman.md) :-)

### Prolog

I don't want this to be introduction to Prolog, but more like simplified version of DCGs. Think about DCGs as a definition of the grammars that can either digest or output tokens (well formed DCGs can do both of those!). For `pikchr_pl` the subset you might be interested in is generation, so I'll skip straight to it:

```prolog
hello --> "Hello".
world --> "World".
space --> " ".

greeting --> hello, space, world.
```

Voilá, here's our greeting. Let's change it to greeting someone!

```prolog
greeting(Who) --> hello, space, Who, "!".
```

If you'd be running Prolog interpreter I'd advise something like `phrase(greeting("John"), Output), format("~s", [Output]).` but this exactly the part `pikchr_pl` is taking care for you (of course, assuming that you're generating a diagram :)).

When searching for resources focus on DCG, because general Prolog, while nice and powerful, isn't easy to learn. Note that LLMs are quite good at generating Prolog code, so you might want to learn with them, just remember to remind them that you want DCGs.
## Attribution

TODO, for now see included LICENSE and attribution files.

## License

- Code is licensed with GPLv3 (SPDX: `GPL-3.0-only`)
- Trealla, Pikchr and Font have separate licenses (See included files)
- 

## TODO

- [ ] Add building/release to CI
- [ ] Add in-app help
- [ ] Add preview of Pikchr code
- [ ] Add menus (for modes, about/licenses)
- [ ] Add SVG/PNG exports (needs menus)
- [ ] Allow adjustment of preview/code area 
- [ ] Groom README.md
- [ ] Add more examples (both source code and images)
## Examples

Note: These were taken at different stage of development.
![grid](https://github.com/exlee/pikchr.pl/blob/master/examples/grid.png)
![file](https://github.com/exlee/pikchr.pl/blob/master/examples/file.png)
![items](https://github.com/exlee/pikchr.pl/blob/master/examples/items.png)
![commits](https://github.com/exlee/pikchr.pl/blob/master/examples/commits.png)
![circles](https://github.com/exlee/pikchr.pl/blob/master/examples/circles.png)
![events](https://github.com/exlee/pikchr.pl/blob/master/examples/events.png)

[Pikchr]: https://pikchr.org
[Trealla]: https://github.com/trealla-prolog/trealla
[Trealla Prolog]: https://github.com/trealla-prolog/trealla
