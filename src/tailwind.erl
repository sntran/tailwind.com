-module(tailwind).
-moduledoc """
Install and run tailwind.com, the Tailwind CSS compiler in one file.

Configuration (the application environment of `tailwind`):

- `{version, "4.3.5"}`: the version of Tailwind CSS. One tailwind.com has
  each version of Tailwind CSS 4.
- `{path, "PATH"}`: the path of tailwind.com (default:
  `"_build/tailwind.com"`).
- `{url, "URL"}`: the URL of the download. `$program` in it is the
  version of this package, and `$version` is the version of Tailwind CSS.
- `{sha256, "HEX"}`: the SHA-256 that the download must have.
- `{PROFILE, [{args, [...]}, {cd, "DIR"}, {env, #{...}}]}`: the arguments
  of a profile for `run/2`.
- `{nif, true | false | "PATH"}`: the program as a NIF (the module
  `tailwind_nif`). `true` (the default): the NIF when it is in `priv`,
  else the file. `false`: the file only. `"PATH"`: the NIF at this path,
  else the file.

The module needs Erlang/OTP 29 or later.
""".


-export([
    bin_path/0,
    bin_version/0,
    command/1,
    config_for/1,
    configured_version/0,
    default_base_url/0,
    install/0,
    install/1,
    latest_version/0,
    nif/0,
    run/2
]).

-define(LATEST, "4.3.5").
-define(BASE_URL, "https://github.com/sntran/tailwind.com/releases/download/v$program/tailwind.com").
%% The variable that gives the version of Tailwind CSS to tailwind.com.
-define(VERSION_VARIABLE, "TAILWINDCSS_VERSION").

-type profile() :: atom().

-doc """
The version of Tailwind CSS when the configuration has no version: the
version of the Rust compiler of tailwind.com.
""".
-spec latest_version() -> string().
latest_version() -> ?LATEST.

-doc """
The version of the configuration.
""".
-spec configured_version() -> string().
configured_version() ->
    to_list(application:get_env(tailwind, version, ?LATEST)).

-doc """
The default URL of the download. `$program` in it is the version of this
package, and `$version` is the version of Tailwind CSS of the
configuration. One file has each version of Tailwind CSS 4, so the default
URL has only `$program`.
""".
-spec default_base_url() -> string().
default_base_url() -> ?BASE_URL.

-doc """
The configuration of a profile, as a map with `args`, `cd` and `env`.
""".
-spec config_for(profile()) -> #{args := [string()], cd := string(), env := #{string() => string()}}.
config_for(Profile) when is_atom(Profile) ->
    case application:get_env(tailwind, Profile) of
        {ok, Config} ->
            Map = if is_map(Config) -> Config; true -> maps:from_list(Config) end,
            {ok, Cwd} = file:get_cwd(),
            #{
                args => [to_list(A) || A <- maps:get(args, Map, [])],
                cd => to_list(maps:get(cd, Map, Cwd)),
                env => maps:from_list(
                    [{to_list(K), to_list(V)} || {K, V} <- maps:to_list(to_map(maps:get(env, Map, #{})))]
                )
            };
        undefined ->
            erlang:error({unknown_tailwind_profile, Profile})
    end.

-doc """
The path of tailwind.com.
""".
-spec bin_path() -> string().
bin_path() ->
    case application:get_env(tailwind, path) of
        {ok, Path} -> to_list(Path);
        undefined -> filename:absname(filename:join("_build", "tailwind.com"))
    end.

-doc """
True when a run uses the NIF (`tailwind_nif`) and not the file
tailwind.com: the configuration `nif` permits it, and the NIF loads.
""".
-spec nif() -> boolean().
nif() ->
    case application:get_env(tailwind, nif, true) of
        false -> false;
        true -> tailwind_nif:available();
        Path -> tailwind_nif:load(Path) =:= ok
    end.

-doc """
The executable and its arguments for a run of the program.

tailwind.com is an Actually Portable Executable (its first bytes are
`MZ`): on Windows it is a program, and on the other systems the shell
starts it, because the kernel does not start such a file in all cases. A
file of another type (a program for this system only) starts directly.
""".
-spec command([string()]) -> {string(), [string()]}.
command(Args) ->
    Path = bin_path(),
    case {os:type(), is_ape(Path), os:find_executable("sh")} of
        {{win32, _}, _, _} -> {Path, Args};
        {_, true, Sh} when Sh =/= false -> {Sh, [Path | Args]};
        _ -> {Path, Args}
    end.

is_ape(Path) ->
    case file:open(Path, [read, raw, binary]) of
        {ok, File} ->
            Head = file:read(File, 2),
            ok = file:close(File),
            Head =:= {ok, <<"MZ">>};
        _ ->
            false
    end.

-doc """
The version of Tailwind CSS that the installed file (or the NIF) uses for
the version of the configuration. The result is `error` when the file is
not there, or when it does not have that version.
""".
-spec bin_version() -> {ok, string()} | error.
bin_version() ->
    case nif() orelse filelib:is_regular(bin_path()) of
        false ->
            error;
        true ->
            Options = [{env, [{?VERSION_VARIABLE, configured_version()}]}],
            try exec(["--version"], Options, fun(Data, Acc) -> [Acc, Data] end, []) of
                {0, Out} ->
                    case re:run(Out, "tailwindcss v([0-9][^\\s\\e,)]*)", [{capture, all_but_first, list}]) of
                        {match, [Vsn]} -> {ok, Vsn};
                        nomatch -> error
                    end;
                _ ->
                    error
            catch
                _:_ -> error
            end
    end.

-doc """
Run tailwind.com with the arguments of the profile and the extra
arguments.

The program compiles for the version of Tailwind CSS of the
configuration: it gets the variable `TAILWINDCSS_VERSION`, unless the
`env` of the profile has that variable.

The output goes to the group leader. The result is the exit status. When
this process ends, the standard input of the program closes, and a watch
(`--watch`) stops.

When `nif/0` is true, the program runs in the VM (`tailwind_nif:run/2`),
with the same arguments, output and result.
""".
-spec run(profile(), [string()]) -> non_neg_integer().
run(Profile, ExtraArgs) when is_atom(Profile), is_list(ExtraArgs) ->
    #{args := Args, cd := Cd, env := Env} = config_for(Profile),
    Options = [{cd, Cd}, {env, maps:to_list(maps:merge(#{?VERSION_VARIABLE => configured_version()}, Env))}],
    Print = fun(Data, Acc) -> io:put_chars(Data), Acc end,
    {Status, _} = exec(Args ++ [to_list(A) || A <- ExtraArgs], Options, Print, ok),
    Status.

exec(Args, Options, Fun, Acc0) ->
    case nif() of
        true ->
            Run = maps:from_list([{Key, Value} || {Key, Value} <- Options, Key =:= cd orelse Key =:= env]),
            tailwind_nif:run(Args, Run, fun(_Stream, Data, Acc) -> Fun(Data, Acc) end, Acc0);
        false ->
            exec_port(Args, Options, Fun, Acc0)
    end.

exec_port(Args, Options, Fun, Acc0) ->
    {Exe, Argv} = command(Args),
    Port = open_port(
        {spawn_executable, Exe},
        [{args, Argv}, binary, exit_status, stderr_to_stdout, use_stdio, hide | Options]
    ),
    loop(Port, Fun, Acc0).

loop(Port, Fun, Acc) ->
    receive
        {Port, {data, Data}} -> loop(Port, Fun, Fun(Data, Acc));
        {Port, {exit_status, Status}} -> {Status, Acc}
    end.

-doc """
Download tailwind.com to `bin_path/0`, from the URL of the configuration.
""".
-spec install() -> ok | {error, term()}.
install() ->
    install(to_list(application:get_env(tailwind, url, ?BASE_URL))).

-doc "Download tailwind.com to `bin_path/0`, from this URL.".
-spec install(string() | binary()) -> ok | {error, term()}.
install(BaseUrl) ->
    Url0 = re:replace(to_list(BaseUrl), "\\$version", configured_version(), [global, {return, list}]),
    Url = re:replace(Url0, "\\$program", program_version(), [global, {return, list}]),
    Path = bin_path(),
    Tmp = Path ++ ".download",
    maybe
        {ok, Body} ?= fetch(Url),
        ok ?= check_sha256(Body),
        ok ?= filelib:ensure_dir(Path),
        ok ?= file:write_file(Tmp, Body),
        ok ?= file:change_mode(Tmp, 8#755),
        file:rename(Tmp, Path)
    end.

%% The version of this package: the release that has its tailwind.com.
program_version() ->
    _ = application:load(tailwind),
    case application:get_key(tailwind, vsn) of
        {ok, Vsn} -> to_list(Vsn);
        undefined -> erlang:error(no_tailwind_application)
    end.

check_sha256(Body) ->
    case application:get_env(tailwind, sha256) of
        undefined ->
            ok;
        {ok, Expected} ->
            Actual = binary_to_list(binary:encode_hex(crypto:hash(sha256, Body), lowercase)),
            case string:lowercase(to_list(Expected)) of
                Actual -> ok;
                _ -> {error, {sha256_mismatch, Actual}}
            end
    end.

fetch("file://" ++ Path) ->
    file:read_file(Path);
fetch(Url) ->
    {ok, _} = application:ensure_all_started(inets),
    {ok, _} = application:ensure_all_started(ssl),
    set_proxy(Url),
    Http = [{ssl, ssl_options()}, {autoredirect, true}, {timeout, 300000}],
    case httpc:request(get, {Url, [{"user-agent", "tailwind.com"}]}, Http, [{body_format, binary}]) of
        {ok, {{_, 200, _}, _, Body}} -> {ok, Body};
        {ok, {{_, Status, _}, _, _}} -> {error, {http_status, Status, Url}};
        {error, Reason} -> {error, {Reason, Url}}
    end.

%% The proxy of the environment (HTTPS_PROXY or HTTP_PROXY), as other
%% download tools use it.
set_proxy(Url) ->
    {Names, Option} =
        case Url of
            "https" ++ _ -> {["HTTPS_PROXY", "https_proxy"], https_proxy};
            _ -> {["HTTP_PROXY", "http_proxy"], proxy}
        end,
    case [V || N <- Names, V <- [os:getenv(N)], V =/= false, V =/= ""] of
        [Proxy | _] ->
            case uri_string:parse(Proxy) of
                #{host := Host} = Uri ->
                    Port = maps:get(port, Uri, 80),
                    _ = httpc:set_options([{Option, {{Host, Port}, []}}]),
                    ok;
                _ ->
                    ok
            end;
        [] ->
            ok
    end.

ssl_options() ->
    CaCerts =
        case os:getenv("TAILWIND_CACERTFILE") of
            false -> public_key:cacerts_get();
            File -> File
        end,
    Verify = [
        {verify, verify_peer},
        {depth, 4},
        {customize_hostname_check, [{match_fun, public_key:pkix_verify_hostname_match_fun(https)}]}
    ],
    case CaCerts of
        [] -> Verify;
        File2 when is_list(File2), is_integer(hd(File2)) -> [{cacertfile, File2} | Verify];
        Certs -> [{cacerts, Certs} | Verify]
    end.

to_list(V) when is_binary(V) -> binary_to_list(V);
to_list(V) when is_atom(V) -> atom_to_list(V);
to_list(V) when is_list(V) -> V.

to_map(M) when is_map(M) -> M;
to_map(L) when is_list(L) -> maps:from_list(L).
