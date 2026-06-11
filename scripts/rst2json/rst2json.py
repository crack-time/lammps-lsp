#!/usr/bin/env python3
"""
Convert LAMMPS RST documentation to commands.json for the lammps-lsp.
Adapted from rst2JSON.py in lammps_vscode (MIT license derived).
"""

import json
import os
import re
import logging
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rst2md


class CMD:
    """Class representing the contents of a documentation page for LAMMPS Commands"""

    def __init__(self, rst_path, ix_syntax=0):
        self.__rst_path__ = rst_path
        self.__rst_txt__ = ""
        self.__sections__ = {
            "Syntax": "",
            "Examples": "",
            "Description": "",
            "Restrictions": "",
            "Related commands": "",
            "Default": "",
        }
        self.__ix_syntax__ = ix_syntax
        self.cmd_list = []
        self.cmd_acc_var = []
        self.valid = [False, False]
        self.syntax = ""
        self.n_syntax = 1
        self.parameters = ""
        self.args = []
        self.examples = ""
        self.html_filename = ""
        self.short_description = ""
        self.description = ""
        self.restrictions = ""
        self.related = ""
        self.references = []
        self.__build__()

    def __build__(self):
        self.__read_file2sections__()
        self.__validate__()
        if self.valid[0]:
            self.__get_refs__()
            self.__get_syntax_prms__()
            self.__get_args__()
            self.__get_html_link__()
            self.examples = self.__section2md__(self.__sections__["Examples"])
            self.description = self.__section2md__(self.__sections__["Description"])
            self.__get_short_description__()
            self.restrictions = self.__section2md__(self.__sections__["Restrictions"])
            if len(self.__section2md__(self.__sections__["Related commands"])) > 0:
                self.related = self.__section2md__(
                    self.__sections__["Related commands"]
                )
            else:
                self.related = "none"

    def __read_file2sections__(self):
        with open(self.__rst_path__, "r") as f:
            self.__rst_txt__ = f.read()
            splits = []
            section_re = re.compile(
                r"^(Syntax|Examples|Example|Description|Restrictions|Related.*?commands|Default)\s*\n(?:[\"'`\-=_~^+*#]{2,}\s*\n)?",
                re.MULTILINE,
            )
            for m in section_re.finditer(self.__rst_txt__):
                tag_raw = m.group(1)
                tag = "Examples" if tag_raw == "Example" else tag_raw
                splits.append({"tag": tag, "p1": m.start(), "p2": m.end()})

            n_splits = len(splits)
            if n_splits > 0:
                splits = sorted(splits, key=lambda x: x["p1"])
                self.__sections__.update(
                    {"Indices": self.__rst_txt__[0 : splits[0]["p1"]]}
                )
                for xs in range(n_splits):
                    x1 = splits[xs]["p2"]
                    x2 = (
                        len(self.__rst_txt__)
                        if xs == n_splits - 1
                        else splits[xs + 1]["p1"]
                    )
                    text = (
                        self.__sections__[splits[xs]["tag"]] + self.__rst_txt__[x1:x2]
                    )
                    self.__sections__.update({splits[xs]["tag"]: text})

    def __validate__(self) -> bool:
        if "Indices" in self.__sections__:
            txt_block = re.sub(r"(`)|(\s+\<.*\>)", "", self.__sections__["Indices"])
            txt_block = re.sub(r"\.\.\ index::.*\n", "", txt_block)
            cmds = re.findall(
                r"((?:\n+)([ \t\S\n]*?)\s+command\n+\=+\n*(.*)(?=\n+.*\n+\=+\n*))",
                txt_block + "\n=\n",
            )
            if cmds:
                b_commands = True
                for ic, cmd in enumerate(cmds):
                    cmd = list(filter(None, cmd))
                    if not cmd[1].__contains__(":doc:"):
                        self.cmd_list.append(cmd[1])
                        if len(cmd) >= 3:
                            if cmd[2].__contains__("Accelerator Variants"):
                                acc_var = re.split(
                                    r",\s*",
                                    cmd[2].replace("Accelerator Variants: ", ""),
                                )
                                for a in acc_var:
                                    a = a.replace("*", "")
                                    cmd_acc_var = cmd[1].replace(a[0 : a.rfind("/")], a)
                                    self.cmd_list.append(cmd_acc_var)
                                    self.cmd_acc_var.append(a)
            else:
                b_commands = False
        else:
            b_commands = False

        b_complete = not (
            not self.__sections__["Syntax"]
            or not self.__sections__["Examples"]
            or not self.__sections__["Description"]
        )

        self.valid = [b_commands, b_complete]

    def __get_html_link__(self):
        rst_path = os.path.basename(self.__rst_path__)
        self.html_filename = rst_path.replace(".rst", ".html")

    def __get_short_description__(self):
        desc = self.description
        equations = []

        def save_and_replace_display(match):
            equations.append(("display", match.group(0)))
            return f"___EQ{len(equations) - 1}___"

        desc = re.sub(r"\\\[[\s\S]*?\\\]", save_and_replace_display, desc)

        def save_and_replace_inline(match):
            equations.append(("inline", match.group(0)))
            return f"___EQ{len(equations) - 1}___"

        desc = re.sub(r"\\\([\s\S]*?\\\)", save_and_replace_inline, desc)
        parts = re.split(r"\.\s+", desc, 1)
        short_desc = parts[0] if parts else desc
        for i, (eq_type, eq_text) in enumerate(equations):
            short_desc = short_desc.replace(f"___EQ{i}___", eq_text)
        self.short_description = short_desc

    def __get_syntax_prms__(self):
        def tweak_prms_block(blocks: str) -> str:
            out = ""
            for b in blocks:
                if re.match(r"^\s*\.\.\s*spacer\s*$", b.strip()):
                    continue
                if b.__contains__("parsed-literal::") or b.__contains__("code-block::"):
                    lines = rst2md.tr_plain(b).splitlines()
                    lines = [ln for ln in lines if ln]
                    leading_ws = [len(l) - len(l.lstrip()) for l in lines]
                    ws_min = min(leading_ws) if leading_ws else 0
                    for ix, l in enumerate(lines):
                        l_str = l[:ws_min].replace(" ", "&#160;") + l[ws_min:] + "   \n"
                        if leading_ws[ix] == ws_min:
                            out += "  * " + l_str
                        else:
                            out += "    " + l_str
                else:
                    out += re.sub(r"(?m)^\s*\.\.\s*spacer\s*$\n?", "", b) + "   \n"
            out = rst2md.rm_markup(out)
            out = rst2md.tr_inline_math(out)
            out = rst2md.fix_tr_markup_bugs(out)
            return out

        def get_multiple_syntaxes(syn_clean):
            syntaxes = []
            lines = syn_clean.splitlines()
            cmd_word = lines[0].split(" ")[0].strip()
            for l in lines:
                if l.strip().split(" ")[0].strip() == cmd_word:
                    syntaxes.append(l.strip())
            return syntaxes

        blocks = self.__section2blocks__(self.__sections__["Syntax"])
        syntax_clean = rst2md.tr_plain(blocks[0]).replace("*", "").strip()
        syntax_array = get_multiple_syntaxes(syntax_clean)
        self.syntax = syntax_array
        self.n_syntax = len(syntax_array)
        prms_block = tweak_prms_block(blocks[1:])
        self.parameters = rst2md.tr_inline_link(rst2md.tr_inline_doc(prms_block))

    def __blocks2md_section__(self, blocks: str) -> str:
        out = str()
        for ib in range(0, len(blocks)):
            out += rst2md.tr_blocks(blocks[ib]) + "\n"
        return out

    def __section2blocks__(self, txt: str) -> str:
        return rst2md.split_directive_blocks(txt)

    def __section2md__(self, section_rst: str) -> str:
        blocks = self.__section2blocks__(section_rst)
        section_md = self.__blocks2md_section__(blocks)
        return rst2md.tr_section(section_md, self)

    def __get_args__(self):
        def clean_dscp_strings(dscps):
            temp_prms = []
            for dscp in dscps:
                level = (dscp.count("&#160;") // 3) + 1
                dscp_args = (
                    dscp[0 : dscp.find("=")]
                    .replace("*", "")
                    .replace("&#160;", "")
                    .replace("(optional)", "")
                    .strip()
                    .split(",")
                )
                dscp = (
                    dscp[dscp.find("=") + 1 :]
                    .replace("(optional)", "")
                    .strip()
                    .replace("0/1", "0 or 1")
                )
                for dscp_arg in dscp_args:
                    temp_prms.append([dscp_arg, dscp, level])
            return temp_prms

        def getChoices(arg: str, dscps: str) -> list:
            choices: str = []
            a_type = 2
            arg_clean = re.sub(r"[\[\]\*\<\>]", "", arg)
            for dscp in dscps:
                if dscp[0].strip().split(" ")[0] == arg_clean and dscp[2] == 1:
                    dscp_fil = re.sub(r"(one|zero)\sor\smore(\sof)?\s*", "", dscp[1])
                    b_choices = re.search(r".*\s(or)\s", dscp_fil) != None
                    if b_choices:
                        choices = dscp_fil.split(" or ")
                        choices = [
                            x.strip()
                            for x in choices
                            if not x.__contains__("=") and not x.__contains__(" ")
                        ]
                        a_type = 3
                        if len(choices) <= 1:
                            a_type = 2
                            choices = []
                    else:
                        a_type = 2
            return a_type, choices

        def args_standard_commands(self, args, dscps):
            def app_choices(a, choices):
                b_acc = False
                for ac in self.cmd_acc_var:
                    base_cmd = ac[0 : ac.rfind("/")]
                    if base_cmd == a:
                        b_acc = True
                        choices.append(a.replace(ac[0 : ac.rfind("/")], ac))
                return choices, b_acc

            com_words = self.cmd_list[0].strip().split(" ")
            arg_ar = []
            for a in args:
                choices = []
                a_clean = re.sub(r"[\[\]\*\<\>]", "", a, 8)
                if com_words.__contains__(a):
                    choices, b_acc = app_choices(a, choices)
                    if b_acc:
                        choices.insert(0, a)
                        arg_ar.append({"arg": a, "type": 3, "choices": choices})
                    else:
                        arg_ar.append({"arg": a, "type": 1, "choices": choices})
                else:
                    a_type, choices = getChoices(a, dscps)
                    for a in choices:
                        choices, b_acc = app_choices(a, choices)
                    arg_ar.append({"arg": a, "type": a_type, "choices": choices})
            return arg_ar

        def args_AtC_commands(self, args, dscps):
            arg_ar = []
            choices: str = []
            for a in args:
                a_clean = re.sub(r"[\[\]\*\<\>]", "", a, 8)
                if a.startswith(("<", "[")) and a.endswith(("]", ">")):
                    if a.__contains__("|"):
                        choices = a_clean.split("|")
                        a_type = 3
                    else:
                        a_type, choices = getChoices(a, dscps)
                else:
                    a_type = 1
                    choices = []
                arg_ar.append({"arg": a, "type": a_type, "choices": choices})
            return arg_ar

        for s in self.syntax:
            args = re.split(r"(?<!AtC)\s", s.strip())
            args = [x for x in args if x != "..."]
            dscps = clean_dscp_strings(self.parameters.splitlines())
            b_AtC = self.syntax[0].__contains__("AtC")
            if b_AtC:
                arg_ar = args_AtC_commands(self, args, dscps)
            else:
                arg_ar = args_standard_commands(self, args, dscps)
            self.args.append(arg_ar)

    def __get_refs__(self):
        refs = re.findall(r"(\:ref\:\`?\(?(.*?)\)?\s*\<(.*?)\>\`?)", self.__rst_txt__)
        for r in refs:
            tag = r[2]
            tag_rst = r[0]
            tag_md = "**" + tag + "**"
            fn = re.search(
                rf"\.\.\s*\_{tag}\:\n+([\s\S\r]*?)(?:\n\n+?|\s*?$|[\s\n]*?\.\.)",
                self.__rst_txt__,
            )
            if fn:
                fn_rst = fn[0]
                fn_md = fn[1]
                fn_md = re.sub(r"\*\*\(.*?\)\*\*", "**(" + tag + ")**", fn_md) + " \n "
                self.references.append(
                    {
                        "tag_rst": tag_rst,
                        "fn_rst": fn_rst,
                        "tag_md": tag_md,
                        "fn_md": fn_md,
                    }
                )


def main():
    logging.basicConfig(level=logging.INFO, format="%(levelname)s: %(message)s")

    rst_path = Path("./rst")
    if not rst_path.exists():
        print(f"Error: RST directory not found at {rst_path.absolute()}")
        print("Usage: python3 rst2json.py")
        print("Place LAMMPS RST docs in ./rst/ directory")
        sys.exit(1)

    rst_files = sorted([f.name for f in rst_path.iterdir() if f.suffix == ".rst"])
    cmd_count = 0
    warning_cmds = []
    all_cmds = []

    for rst in rst_files:
        Doc = CMD(os.path.join(rst_path, rst))
        if Doc.valid[0]:
            cmd_count += len(Doc.cmd_list)
            entry = {
                "command": Doc.cmd_list,
                "syntax": Doc.syntax,
                "args": Doc.args,
                "parameters": Doc.parameters,
                "examples": Doc.examples,
                "html_filename": Doc.html_filename,
                "short_description": Doc.short_description,
                "description": Doc.description,
                "restrictions": Doc.restrictions,
                "related": Doc.related,
            }
            all_cmds.append(entry)
            logging.info("passed: %s (%d commands)", rst, len(Doc.cmd_list))
            if not Doc.valid[1]:
                warning_cmds.append(rst)
        else:
            logging.debug("skipped: %s", rst)

    out_path = Path("../commands.json")
    with out_path.open("w", encoding="utf-8") as f:
        json.dump(all_cmds, f, ensure_ascii=False, indent=2)

    logging.info(
        "Wrote %d entries (%d commands) to %s",
        len(all_cmds),
        cmd_count,
        out_path.absolute(),
    )
    if warning_cmds:
        logging.warning(
            "Incomplete entries found for %s command(s): %s",
            len(warning_cmds),
            ", ".join(warning_cmds),
        )


if __name__ == "__main__":
    main()
