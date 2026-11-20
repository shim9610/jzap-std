# macOS 전체 성능 벤치마크 프롬프트

아래 인용문을 Mac에서 실행할 에이전트에게 그대로 전달한다.

> `shim9610/jzap-std`의 `0.1.0-beta.1` 배포 후보를 실제 macOS 머신에서 벤치마크하라. 이번 작업은 배포 후보의 기준 성능 측정이다. 제품 소스와 버전, 릴리스 태그는 고정하라. Darwin 호환 수정이 필요하면 벤치 도구에 적용하고 변경 사항을 결과와 함께 기록하라.
>
> ## 후보와 실험 환경 고정
>
> 1. 저장소의 `AGENTS.md`, `docs/support.md`, `docs/benchmarks.md`, `tools/throughput-analysis/README.md`, `docs/btree.md`의 B+ 벤치 구간을 읽어라. 사용자가 지정한 배포 후보 SHA를 사용하라. 지정이 없으면 작업 시작 시점의 `origin/main`에서 `0.1.0-beta.1`을 확인해 그 SHA를 고정하라. 소스 변경이 있는 작업 트리는 보존하라.
> 2. `git rev-parse HEAD`, `sw_vers`, `uname -m`, CPU 모델과 코어 구성, 메모리, 전원 상태, `rustc +1.95.0 -Vv`, `cargo +1.95.0 -V`, `xcodebuild -version`을 기록하라. 네이티브 Mach-O 바이너리를 사용하고 아키텍처마다 별도 결과를 남겨라. Rust는 1.95.0, Cargo는 `--locked`, 기본 release 플래그를 기준으로 한다. 바이너리 SHA-256과 옵션을 먼저 기록하고 빌드·테스트·형식검증이 끝난 뒤에 벤치를 시작하라.
> 3. 첫 빌드는 아래처럼 수행하라. `arc_scenarios`는 Linux 전용 호출이 있으므로 별도 대상으로 점검하라. 다른 분석용 작업공간은 각 작업공간의 매니페스트와 기존 재현 문서를 사용하라.
>
>    ```sh
>    cargo +1.95.0 build --locked --release \
>      --manifest-path tools/throughput-analysis/Cargo.toml \
>      --target-dir target/verification/macos/bin \
>      --features hash-map,hash-set,owned-budgets,bplus-pool \
>      --bin jzap-throughput-analysis --bin string --bin shared \
>      --bin btree --bin btree_set --bin hash_map --bin hash_set \
>      --bin binary_heap --bin deque --bin scratch_slice --bin bplus --bin scenarios
>    ```
>
> ## Darwin에서 측정 도구 확인
>
> - `compare.py`의 `os.sched_*affinity`, `arc_scenarios`의 `/proc`과 `sched_setaffinity`, 재현 명령의 `taskset`은 Linux 전용이다. Darwin에서는 CPU 고정 없이 실행한다는 점을 프로토콜에 명시하라. 스레드 수, 동기화, 시드별 입력과 반복 순서는 양 구현에서 동일하게 유지하라. 도구를 고칠 때 최소 변경으로 Linux 경로와 기존 연산 정의를 보존하라. 호환 경로를 검증하지 못한 조건은 실행 여부와 원인을 표에 남겨라.
> - 메모리 관측은 시간 측정과 별도 빌드에서 실행하라. glibc의 `malloc_usable_size`가 들어간 도구를 찾고, Darwin에서는 지원되는 할당의 `malloc_size` 또는 요청 바이트 계측을 선택하라. 관측한 수치가 요청량, 할당자 사용 가능 크기, 프로세스 상주 메모리(RSS) 중 무엇인지 구분하라. 재할당 실패와 해제에서 집계가 맞는지 먼저 작은 대조를 실행하라. 실제 원인 분석에는 `otool` 또는 `llvm-objdump`를 쓰고, 명령·메모리 접근·분기 또는 할당 경로를 확인한 범위만 설명하라.
> - Box의 Varlit 대조는 문서에 고정된 `cd7b0446da3e731fba867802bef5f4b7b3fc328d` 체크아웃이 필요하다. 확보 여부를 기록하고, 별도로 실행 가능한 Box 대 std 측정은 진행하라.
>
> ## 측정 범위와 대조군
>
> - `tasks`나 `describe`를 제공하는 바이너리는 출력을 보존하고, 기존 보고서의 행과 맞춰 실행 계획을 만들어라. 산술·Checked, Box, Vec와 범위 편집(ExtractIf·Splice 포함), String, Rc/Arc/Weak 및 동시 Arc 시나리오, 예산 정책, BTreeMap/BTreeSet, HashMap/HashSet, BinaryHeap, VecDeque, Scratch/checked slice, B+를 포함하라. 각 조건에 연산, 입력 규모, 성공·실패, 정책, 준비와 소멸을 포함하는지 명시하라. 대응하는 std 연산이 있는 조건은 std 시간을 목표로 삼고, 대응이 없는 기능은 측정 절차와 목표 부재를 기록하라.
> - B+의 목표는 같은 프로세스에서 같은 입력으로 측정한 `brie-tree` **0.1.2** 시간이다. `bplus-pool`의 512, 32,768, 65,536개 키 조건과 공통 `u64` 질의·값, 시드별 4,096개 질의 및 hit/miss 결과를 유지하라. 공개 스트리밍 조회와 공개 `FixedTree` 조회를 각각 표에 넣어라. `resident`, `split`, `packed`도 각각 보고하라. std BTreeMap 대조는 `--features bplus-bench --bin bplus`로 별도 빌드하고 해당 바이너리에서 함께 측정한 `brie-tree` 시간을 사용하라. 조회 전에 끝나는 검증·구축은 별도 표로 기록하라. 구축 비용은 문서의 `BPLUS_PREPARE=1` 모드를 사용하라. 양쪽 실제 트리 높이와 노드 폭은 원자료에 남겨라.
> - 모든 비교는 같은 바이너리 설정, 입력 순서, 연산 수, 정책 조건, 준비·소멸 범위에서 짝지어라. B+는 공통 배치 수로 가장 빠른 쪽도 1초 이상 걸리도록 보정한 뒤, `BPLUS_BATCHES_<fanout>_<height>`를 기록하고 `FixedTree`만 보는 진단과 전체 경로 측정을 별도 실행하라.
>
> ## 반복, 판정, 전달
>
> - 조건을 고정한 뒤 시드마다 새 프로세스로 실행하고 순서를 회전·반전하라. 각 유효 구간은 1초 이상이고, 한 쌍의 연산 수는 같다. B+는 시드 1, 2, 3, 10을 시작점으로 사용하고, 다른 도구는 문서에 있는 시드·계획을 사용하라. 같은 구현을 독립 프로세스에서 반복하는 대조(A/A)를 사용하고 원시 시드별 시간, 연산 수, 체크섬, 짧은 구간과 실패 기록을 남겨라. 대조군 드리프트와 표본 간 편차를 확인하고 불안정한 결과는 판정 보류로 표기하라. 재시도는 설정과 바이너리 해시가 같은 경우 같은 기록에 추가하라.
> - 주 보고표 열은 **조건 | 비교군 시간 | jzap 시간 | 목표와의 차이**로 고정하라. B+ 표의 비교군 열 이름은 **`brie-tree` 시간**이다. 차이는 `jzap 시간 − 같은 조건의 비교군 시간`이며 단위와 필요하면 상대 차이도 표시하라. 원시 총시간과 연산 수에서 연산당 시간을 계산하고, 최소·최대 또는 신뢰구간 등 편차는 표 아래에 기록하라. 서로 다른 조건을 하나의 점수로 합치지 말라. 실행하지 못한 조건, 실패, 판정 보류를 누락하지 말라.
> - `target/verification/macos/`에 환경·프로토콜·바이너리 해시·원시 CSV/JSONL·요약을 남겨라. 최종 보고에는 실제 실행 명령, 원자료 경로, 조건별 표, 측정하지 못한 범위가 있어야 한다. 목표 차이가 큰 조건을 진단했다면 근거를 붙이고, 원인을 확인하지 못했으면 추정과 관측을 구분하라. 실제 배포나 제품 성능 튜닝은 이 측정 작업의 범위 밖이다.
