#!/usr/bin/env python3
"""票 03 的合成 store 生成器（一次性产物，不是产品代码）。

本机真实 store（281 会话 / 88,469,279 B / 74,803 事件）足够说明「小会话多、几条巨流」
的形状，但会话数不够验证「几十个桶、几百条流」的聚合成本。这个脚本按同样的形状放大到
约 280 条流 / 1.4 亿字节 / 29 万事件，并刻意让 10% 的 `SessionStarted.cwd` 指向不存在的
路径（模拟 worktree 已删 / 跨机器搬来的会话）。

用法（在仓库根目录）：
    python3 .scratch/session-tab/research/bench/synth_store.py .cache/session-tab-synth-store

确定性：`random.seed(20261012)`，同样的参数生成同样的内容。
"""

import datetime
import json
import pathlib
import random
import sys

BASE = datetime.datetime(2026, 9, 1, tzinfo=datetime.timezone.utc)


def main() -> None:
    root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".cache/session-tab-synth-store")
    if root.exists():
        import shutil

        shutil.rmtree(root)
    root.mkdir(parents=True)
    random.seed(20261012)

    total_events = total_bytes = 0
    sessions = 0
    for b in range(30):
        # 每 10 个桶里有 1 个的 cwd 指向一个不存在的路径。
        cwd = f"/gone/worktree-{b}" if b % 10 == 3 else f"/home/forty/code/synth/project-{b}"
        bucket = root / (cwd.replace("/", "-") + "-" + format(b * 0x9E3779B97F4A7C15 % (1 << 64), "016x"))
        for _ in range(random.choice([1, 1, 2, 3, 5, 8, 12, 20, 40])):
            n_events = random.choice([5, 9, 12, 18, 30, 60, 120, 300, 600, 1200, 3000, 8000])
            sid = (BASE + datetime.timedelta(minutes=sessions)).strftime("%Y%m%dT%H%M%SZ") + "-" + format(
                random.getrandbits(32), "08x"
            )
            d = bucket / sid
            (d / "outputs").mkdir(parents=True, exist_ok=True)
            lines: list[str] = []
            seq = 1
            at = BASE + datetime.timedelta(minutes=sessions)

            def ev(payload: dict, dt: float = 2.5) -> None:
                nonlocal seq, at
                at += datetime.timedelta(seconds=dt)
                lines.append(
                    json.dumps(
                        {
                            "seq": seq,
                            "at": at.strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z",
                            "speaker_id": "System",
                            "payload": payload,
                        },
                        ensure_ascii=False,
                    )
                )
                seq += 1

            ev({"SessionStarted": {"session_id": sid, "cwd": cwd, "schema_version": 1}})
            ev({"ContextInjected": {"source": "AgentsMd", "content": "x" * random.randint(200, 2000)}})
            ev({"ContextInjected": {"source": "SkillsCatalog", "content": "y" * random.randint(100, 1500)}})
            # 一半的会话在第 4 行就有第一句人话（真实 store 的众数就是第 4-7 行）。
            if random.random() < 0.5:
                ev({"MessageCompleted": {"role": "User", "text": "帮我改一下 " + "z" * random.randint(5, 60), "reasoning": None}})
            while seq < n_events:
                ev({"TurnStarted": {"agent": {"Debater": "main"}, "iteration": random.randint(1, 20)}})
                for _ in range(random.randint(1, 3)):
                    ev({"MessageCompleted": {"role": "Assistant", "text": "这一段是正文。" * random.randint(3, 120), "reasoning": None}})
                    cid = f"call_{seq}"
                    ev({"ToolCallStarted": {"tool_call_id": cid, "tool_name": "read_file", "args": {"path": f"src/file{random.randint(1, 50)}.rs"}}})
                    ev({"PermissionAsked": {"request_id": f"req_{seq}", "tool_call_id": cid, "request": {"tool": "read_file"}}})
                    ev({"PermissionDecided": {"request_id": f"req_{seq}", "decision": "Allow", "source": "Policy", "reason": None}})
                    ev({"ToolCallCompleted": {"tool_call_id": cid, "ok": True, "output": "行" * random.randint(10, 400), "error": None, "duration_ms": random.randint(1, 900)}})
                # 少数会话在中途才有第一句人话（真实 store 里的 685 / 2549 / 2737 / 13195 行）。
                if random.random() < 0.02:
                    ev({"MessageCompleted": {"role": "User", "text": "再改一处 " + "z" * random.randint(5, 60), "reasoning": None}})
                ev({"UsageRecorded": {"usage": {"input_tokens": random.randint(100, 90000), "output_tokens": random.randint(10, 4000), "cached_tokens": random.randint(0, 40000), "miss_tokens": random.randint(0, 40000), "reasoning_tokens": random.randint(0, 2000)}}})
                ev({"TurnEnded": {"reason": "Completed"}})
            if random.random() < 0.15:
                ev({"SessionEnded": {"reason": "Completed"}})
            body = ("\n".join(lines) + "\n").encode()
            (d / "log.jsonl").write_bytes(body)
            total_events += len(lines)
            total_bytes += len(body)
            sessions += 1
    print(f"sessions={sessions} events={total_events} bytes={total_bytes} root={root}")


if __name__ == "__main__":
    main()
