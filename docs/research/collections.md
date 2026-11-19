# 실패 가능한 컬렉션 구현 조사

조사일: 2026-11-03 KST. 배포 소스의 버전·SHA-256은 sources.json에 기록했다.
크레이트의 설명과 실제 검토한 코드의 결과를 구분한다.

| 후보 | 확인한 구현 | 채택 판단 |
| --- | --- | --- |
| Rust 1.95 std BTreeMap | 차수 6, leaf/internal 분리, 부모 포인터와 노드 분할. 노드 생성은 실패 불가능한 Box 경로 | 구조·동작·성능의 직접 비교 기준 |
| nightly-2026-10-30 std | map::try_insert는 OccupiedError를 반환하며 중복 키를 거부한다. node::LeafNode::new는 Box::new_uninit_in을 사용 | 할당 실패 반환 API로 대체할 수 없음 |
| fallible_collections 0.5.2 | unstable B-tree의 VacantEntry::try_insert는 길이를 먼저 증가시키고 leaf 변경 후 상위 노드를 추가 할당한다. Clone/Extend 등에 expect("Out of Mem")가 있음 | 실패 원자성과 무중단 계약에 맞지 않아 B-tree 원본으로 제외. 코드 검토 결과이며 이번 조사에서 실행 재현한 것은 아님 |
| kernel-btree 0.1.0 | 삽입 전 필요한 형제·루트를 확보하고 거부 시 회수. 입력 반환 API, 무할당 삭제, 부모 포인터 기반 반복자·비재귀 정리 | 신규 소규모 프로젝트. 핵심 의존성 채택 보류, 알고리즘 참고만 유지 |
| btree-slab 0.6.1 | slab에서 노드 식별자를 관리하며 insert 반환형은 Option. 일반 저장소 삽입의 할당 실패 반환 계약 없음 | 노드 식별자 분리는 참조하되 직접 채택하지 않음 |
| generic-btree 0.10.7 | arena 기반 일반 트리, std 컨테이너와 panic 경로 사용 | 실패 반환을 제공하는 대체 구현으로 채택하지 않음 |
| heapless 0.9.3 | 고정 용량 insert에서 거부된 입력을 반환 | 입력 반환·외부 용량 계약 참고. 동적 std B-tree의 동등 대체는 아님 |
| rkyv 0.8.18 | ArchivedBTreeMap이 상대 포인터로 보관된 노드를 참조 | 외부 참조 표현 참고. 직렬화 프레임워크·파일 형식은 도입하지 않음 |
| allocator-api2 0.4.0 / 0.2.21 | Box·Vec의 실패 가능한 할당·예약. 별도의 BTreeMap 모듈 없음 | 기본 소유 타입의 차용 후보 유지. 현재 B-tree 의존성에는 추가하지 않음 |

## 선택과 수정 범위

유지보수 조사 후 kernel-btree의 제품 의존성 채택을 철회했다. 공개 저장소는
2026-10-21 생성, 커밋 3개·기여자 1명·별 0·포크 0이며, crates.io 배포는
0.1.0 한 번(10월 28일), 누적 다운로드 10회·등록된 역의존성 0개다.
이 수치는 2026-11-03 KST 조회 시점의 관측이며 비공개 사용 규모는 알 수 없다.
소스 Rust 파일 5개/1,795줄, 테스트 디렉터리 Rust 파일 5개/1,597줄이다.
원격 CI 실행 2회 중 최근 실행만 성공했다. 현재 workflow에는 Miri가 없다.
README의 Miri 실행·std 동등 성능 주장은 독립 실행 결과로 확인하지 않았다.
원본 0.1.0을 stable Rust 1.95.0에서 cargo check --lib --all-features
--offline으로 직접 빌드해 통과했다. MSRV 1.85는 메타데이터 선언이며 이번에
실행한 버전은 1.95다. docs.rs의 nightly 빌드 표시는 nightly 전용이라는 뜻이 아니다.
단기 이력만으로 유지보수 중단이라고 단정할 수 없으나 핵심 의존성 채택의
근거도 부족하다. 소스의 try_clone은 일반 Clone을 호출하므로 jzap 계약과
맞지 않는다. 조사 소스와 임시 B-tree 작업은 workspace scratch에 보존했다.
제품 Cargo 의존성과 공개 모듈에서는 제거했다.

기반 구현 우선순위는 TryClone → 대상 예산 전파 → 초기화·정리 → Box와 중첩
소유권 검증이다. B-tree의 긴급성이 이 선행 조건을 생략하는 근거가 되지 않는다.

범용 슬라이스 트리 뷰는 호출자가 제공한 키/값/자식 식별자 슬라이스만 검사한다.
외부 식별자는 호출자에게 돌려주며 파일 I/O·캐시·직렬화 형식은 다루지 않는다.

## 출처

- https://github.com/rust-lang/rust/blob/1.95.0/library/alloc/src/collections/btree/node.rs
- https://github.com/rust-lang/rust/issues/82766 (map_try_insert의 의미)
- https://docs.rs/fallible_collections/0.5.2/fallible_collections/
- https://docs.rs/kernel-btree/0.1.0/kernel_btree/
- https://github.com/radkum/kernel-btree
- https://docs.rs/btree-slab/0.6.1/btree_slab/
- https://docs.rs/generic-btree/0.10.7/generic_btree/
- https://docs.rs/heapless/0.9.3/heapless/
- https://docs.rs/rkyv/0.8.18/rkyv/collections/btree/map/struct.ArchivedBTreeMap.html
- https://docs.rs/allocator-api2/0.4.0/allocator_api2/
