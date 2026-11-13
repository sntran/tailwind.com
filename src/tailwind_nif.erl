-module(tailwind_nif).
-moduledoc """
The program tailwind.com as a NIF: Tailwind CSS in the Erlang VM, with no
process of the operating system.

The NIF is a choice. This module does not load it at the load of the
module (there is no `-on_load`): it loads the NIF at the first call of
`available/0`, `load/1`, `run/2` or `run/4`. When there is no NIF,
`available/0` is `false`, and the module `tailwind` runs the file
tailwind.com as a port.

The NIF is the library `priv/native/libtailwind_nif` of the application
`tailwind` (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows), from
the crate `native/nif`. A builder can give it at another path: the
configuration `{nif, "PATH"}` of the module `tailwind`, or `load/1`.

A run is a run of the program, with the same arguments and the same
output: `--watch` also works. The NIF has the compiler of Tailwind CSS
4.3.5. For an older version of Tailwind CSS, the NIF uses the npm
packages `tailwindcss` in `priv/tailwindcss` of the application, as the
file tailwind.com uses the packages in its ZIP part: a builder puts them
there (`docs/NIF.md`). The variable `TAILWIND_COM_PACKAGES` can give
another directory. The program runs on a thread of its own,
not on a scheduler of the VM. When the process that calls `run/2` ends,
a watch stops at its next tick, as the program does at the end of its
standard input.

The module needs Erlang/OTP 29 or later. `docs/NIF.md` has more.
""".

-export([available/0, load/1, run/2, run/4, versions/0]).
%% The functions of the NIF.
-export([nif_start/6, nif_versions/0]).

-nifs([nif_start/6, nif_versions/0]).

%% The key of persistent_term when the default NIF is not there.
-define(MISSING, {?MODULE, missing}).

-type stream() :: stdout | stderr.
-type options() :: #{
    cd => file:filename_all(),
    env => [{string() | binary(), string() | binary() | false | nil}] | #{string() | binary() => term()},
    colors => boolean()
}.
-export_type([options/0, stream/0]).

-doc """
True when the NIF is loaded. The first call loads the NIF from the
default path (`priv/native/libtailwind_nif` of the application
`tailwind`). When it is not there, the result is `false`, and the next
calls do not try again: `load/1` can then load a NIF from a path.
""".
-spec available() -> boolean().
available() ->
    loaded() orelse
        (not persistent_term:get(?MISSING, false) andalso
            case load(default_path()) of
                ok ->
                    true;
                {error, _} ->
                    persistent_term:put(?MISSING, true),
                    false
            end).

-doc """
Load the NIF from `Path`, with no extension (as `erlang:load_nif/2`).
When a NIF is already loaded, the result is `ok`.
""".
-spec load(file:filename_all()) -> ok | {error, {atom(), string()}}.
load(Path) ->
    case loaded() of
        true ->
            ok;
        false ->
            case erlang:load_nif(unicode:characters_to_list(Path), 0) of
                ok ->
                    _ = persistent_term:erase(?MISSING),
                    ok;
                Error ->
                    %% Another process can load the NIF at the same time.
                    case loaded() of
                        true -> ok;
                        false -> Error
                    end
            end
    end.

-doc """
The version of the program and the version of Tailwind CSS of its
compiler, as binaries. The error is `{tailwind_nif, not_available}` when
there is no NIF.
""".
-spec versions() -> {binary(), binary()}.
versions() ->
    ensure(),
    nif_versions().

-doc """
Run the program with the arguments `Args`, as `tailwind.com Args`. The
output (the standard output and error) goes to the group leader. The
result is the exit status.

Options:

- `cd`: the working directory of the run (default: the working directory
  of the VM). The working directory of the VM does not change.
- `env`: changes of the environment of the run, as the option `env` of
  `open_port/2`: a value, or `false` (or `nil`) to remove the variable.
  The environment of the VM does not change.
- `colors`: the messages have colors, as on a terminal (default:
  `false`, as for a port).
""".
-spec run([string() | binary()], options()) -> non_neg_integer().
run(Args, Options) ->
    {Status, ok} = run(Args, Options, fun(_Stream, Data, ok) -> io:put_chars(Data), ok end, ok),
    Status.

-doc """
Run the program as `run/2` does, and give each part of the output to
`Fun(Stream, Data, Acc)`. `Stream` is `stdout` or `stderr`, and `Data` is
a binary. The result is the exit status and the last `Acc`.
""".
-spec run([string() | binary()], options(), fun((stream(), binary(), Acc) -> Acc), Acc) ->
    {non_neg_integer(), Acc}.
run(Args, Options, Fun, Acc) when is_list(Args), is_map(Options), is_function(Fun, 3) ->
    ensure(),
    Cwd =
        case maps:find(cd, Options) of
            {ok, Dir} ->
                filename:absname(Dir);
            error ->
                {ok, Dir} = file:get_cwd(),
                Dir
        end,
    Env = packages([{binary(Name), value(Value)} || {Name, Value} <- env(maps:get(env, Options, []))]),
    Id = erlang:unique_integer([positive]),
    Colors = maps:get(colors, Options, false),
    Run = nif_start([binary(Arg) || Arg <- Args], binary(Cwd), Env, self(), Id, Colors),
    loop(Run, Id, Fun, Acc).

%% `Run` is the resource of the run: while this process keeps it, the run
%% goes on. When the garbage collector frees it, a watch stops.
loop(Run, Id, Fun, Acc) ->
    receive
        {?MODULE, Id, exit, Status} ->
            {Status, Acc};
        {?MODULE, Id, Stream, Data} ->
            loop(Run, Id, Fun, Fun(Stream, Data, Acc))
    end.

ensure() ->
    available() orelse erlang:error({?MODULE, not_available}).

loaded() ->
    try nif_versions() of
        {_, _} -> true
    catch
        error:nif_not_loaded -> false
    end.

default_path() ->
    filename:join([priv(), "native", "libtailwind_nif"]).

priv() ->
    case code:priv_dir(tailwind) of
        {error, bad_name} -> filename:join(filename:dirname(filename:dirname(code:which(?MODULE))), "priv");
        Dir -> Dir
    end.

%% The packages of the older versions of Tailwind CSS: `priv/tailwindcss`,
%% when the run and the VM do not give `TAILWIND_COM_PACKAGES`.
packages(Env) ->
    Packages = filename:join(priv(), "tailwindcss"),
    case lists:keymember(<<"TAILWIND_COM_PACKAGES">>, 1, Env) orelse os:getenv("TAILWIND_COM_PACKAGES") =/= false of
        false ->
            case filelib:is_dir(Packages) of
                true -> [{<<"TAILWIND_COM_PACKAGES">>, binary(Packages)} | Env];
                false -> Env
            end;
        true ->
            Env
    end.

env(Env) when is_map(Env) -> maps:to_list(Env);
env(Env) when is_list(Env) -> Env.

value(false) -> nil;
value(nil) -> nil;
value(Value) -> binary(Value).

binary(Value) when is_atom(Value) -> atom_to_binary(Value);
binary(Value) ->
    case unicode:characters_to_binary(Value) of
        Binary when is_binary(Binary) -> Binary;
        _ -> erlang:error(badarg, [Value])
    end.

-doc false.
nif_start(_Args, _Cwd, _Env, _Pid, _Id, _Colors) ->
    erlang:nif_error(nif_not_loaded).

-doc false.
nif_versions() ->
    erlang:nif_error(nif_not_loaded).
