%% The native NIF object of tailwind_nif (scripts/nif.sh) in beam.com:
%% the load, a build, and a watch that stops when its process ends.
%%
%%   sh beam.com tests/nif_object.erl -- build/nif/libtailwind_nif
%%
%% Run it in the root of the repository. The argument is the path of the
%% objects with no CPU and no extension:
%% load_nif/2 of beam.com loads PATH.x86_64.o or PATH.aarch64.o. The
%% exit status is 0 when each check passes.
-module(nif_object).
-export([main/1]).

main([Path]) ->
    {ok, Root} = file:get_cwd(),
    Dir = temp_dir(),
    try
        run(Root, Path, Dir)
    catch
        Class:Reason:Stack ->
            io:format(standard_error, "nif_object: ~p:~p~n~p~n", [Class, Reason, Stack]),
            halt(1)
    after
        _ = file:del_dir_r(Dir)
    end;
main(_) ->
    io:format(standard_error, "usage: beam.com tests/nif_object.erl -- PATH~n", []),
    halt(2).

run(Root, Path, Dir) ->
    {ok, tailwind_nif, Beam} = compile:file(filename:join(Root, "src/tailwind_nif.erl"), [binary, report]),
    {module, tailwind_nif} = code:load_binary(tailwind_nif, "tailwind_nif.erl", Beam),
    check("the object loads", ok, tailwind_nif:load(filename:absname(Path))),
    {Program, Tailwind} = tailwind_nif:versions(),
    io:format("nif_object: tailwind.com ~s (tailwindcss ~s)~n", [Program, Tailwind]),

    Project = filename:join(Dir, "project"),
    ok = filelib:ensure_path(Project),
    ok = file:write_file(filename:join(Project, "app.css"), <<"@import \"tailwindcss\";\n">>),
    ok = file:write_file(filename:join(Project, "index.html"), <<"<p class=\"flex p-4\"></p>\n">>),
    Fold = fun(Stream, Data, Acc) -> [Acc, atom_to_list(Stream), ": ", Data] end,

    %% A build: the CSS goes to the file, the messages to stderr.
    {Status, Out} = tailwind_nif:run(["-i", "app.css", "-o", "out.css"], #{cd => Project}, Fold, []),
    check("a build gives the status 0", 0, Status),
    Css = read(filename:join(Project, "out.css")),
    check("the CSS has .flex", true, has(Css, <<".flex {">>)),
    check("the messages have Done in", true, has(iolist_to_binary(Out), <<"Done in ">>)),

    %% An error of the program gives its status.
    {Status1, _} = tailwind_nif:run(["--version"], #{env => [{"TAILWINDCSS_VERSION", "4.999.0"}]}, Fold, []),
    check("a version that is not there gives 1", 1, Status1),

    %% A JavaScript plugin: QuickJS in the object.
    ok = file:write_file(filename:join(Project, "plugin.js"),
        <<"module.exports = function ({ addUtilities }) { addUtilities({ '.from-js': { color: 'red' } }) }\n">>),
    ok = file:write_file(filename:join(Project, "js.css"),
        <<"@import \"tailwindcss\";\n@plugin \"./plugin.js\";\n">>),
    ok = file:write_file(filename:join(Project, "index.html"), <<"<p class=\"flex from-js\"></p>\n">>),
    {Status2, Out2} = tailwind_nif:run(["-i", "js.css", "-o", "js.out.css"], #{cd => Project}, Fold, []),
    Status2 =:= 0 orelse io:format(standard_error, "~ts~n", [iolist_to_binary(Out2)]),
    check("a build with a plugin gives the status 0", 0, Status2),
    check("the CSS has the class of the plugin", true, has(read(filename:join(Project, "js.out.css")), <<".from-js {">>)),

    %% A watch: a change gives a new output, and the end of the process
    %% stops the watch.
    Self = self(),
    Watch = spawn(fun() ->
        R = tailwind_nif:run(["-i", "app.css", "-o", "watch.css", "--watch"], #{cd => Project},
            fun(_, _, A) -> A end, ok),
        Self ! {done, R}
    end),
    WatchCss = filename:join(Project, "watch.css"),
    check("the watch builds", ok, wait(WatchCss, <<".flex {">>, 200)),
    ok = file:write_file(filename:join(Project, "index.html"), <<"<p class=\"flex grid\"></p>\n">>),
    check("the watch builds again after a change", ok, wait(WatchCss, <<".grid {">>, 200)),
    exit(Watch, kill),
    timer:sleep(1500),
    ok = file:write_file(filename:join(Project, "index.html"), <<"<p class=\"italic\"></p>\n">>),
    timer:sleep(1500),
    check("the watch stops when its process ends", false, has(read(WatchCss), <<".italic {">>)),
    io:format("nif_object: each check passes~n"),
    ok.

check(Name, Expected, Expected) ->
    io:format("nif_object: ~ts: pass~n", [Name]);
check(Name, Expected, Actual) ->
    io:format(standard_error, "nif_object: ~ts: FAIL: ~p, not ~p~n", [Name, Actual, Expected]),
    halt(1).

wait(_File, _Text, 0) ->
    timeout;
wait(File, Text, N) ->
    case file:read_file(File) of
        {ok, Css} ->
            case has(Css, Text) of
                true -> ok;
                false -> timer:sleep(50), wait(File, Text, N - 1)
            end;
        _ ->
            timer:sleep(50),
            wait(File, Text, N - 1)
    end.

read(File) ->
    {ok, Data} = file:read_file(File),
    Data.

has(Data, Text) -> binary:match(Data, Text) =/= nomatch.

temp_dir() ->
    Base = case os:getenv("TMPDIR") of false -> "/tmp"; T -> T end,
    Dir = filename:join(Base, "nif_object." ++ integer_to_list(erlang:unique_integer([positive]))),
    ok = filelib:ensure_path(Dir),
    Dir.
