// Escola de controle: as lições em ordem, com as estrelas de cada uma.

import { Empty, Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { school, useSchool } from '../lib/school'
import { store, useStore } from '../lib/store'

export function School() {
  useSchool()
  const lessons = useStore((s) => s.hello?.lessons ?? [])
  const replay = useStore((s) => s.snap?.replay != null)
  const start = (id: string) => {
    const l = lessons.find((x) => x.id === id)!
    school.start(id)
    store.setUi({ view: 'overview' })
    if (l.scenario) {
      // Não espera a resposta: o "reload" a todas as telas pode chegar antes dela
      // e fechar a conexão. Um erro de verdade (sem permissão…) cancela a lição.
      store
        .sim('scenario', { value: l.scenario })
        .then((r: any) => {
          if (r.type !== 'reload' && r.ok === false) {
            toast(tx(r.error) || t('não deu para carregar o cenário'), 'bad')
            school.stop()
          }
        })
        .catch(() => {})
    }
  }
  return (
    <div className="grid">
      <Panel title={t('Escola de controle')}>
        <p className="muted">
          {t('Missões curtas para aprender a sala: cada uma carrega um cenário e dá objetivos; o Jaci confere sozinho quando você cumpre cada um. Sem dicas e no tempo, três estrelas.')}
        </p>
        {replay && <p className="warn small">{t('Saia da reprodução para começar uma lição.')}</p>}
      </Panel>
      {lessons.length === 0 ? (
        <Empty>{t('Nenhuma lição em missoes/escola.')}</Empty>
      ) : (
        <div className="grid cols-3">
          {lessons.map((l, i) => {
            const stars = school.stars[l.id] ?? 0
            const active = school.run?.id === l.id && !school.run.finished
            return (
              <Panel key={l.id} title={`${i + 1}. ${tx(l.name)}`} right={<span className="stars small">{'★'.repeat(stars)}<span className="muted">{'★'.repeat(3 - stars)}</span></span>}>
                <p className="small">{tx(l.description)}</p>
                <p className="small muted" style={{ whiteSpace: 'pre-line' }}>
                  {tx(l.intro)}
                </p>
                <div className="row">
                  <button className="primary" disabled={replay || active} onClick={() => start(l.id)}>
                    {active ? t('Em andamento') : stars ? t('Fazer de novo') : t('Começar')}
                  </button>
                  <span className="small muted">
                    {l.objectives.length} {t('objetivos')}
                  </span>
                </div>
              </Panel>
            )
          })}
        </div>
      )}
    </div>
  )
}
