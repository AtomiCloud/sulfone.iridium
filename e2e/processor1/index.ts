import { ProcessorOutput, StartProcessorWithLambda } from '@atomicloud/cyan-sdk';
import { Eta } from 'eta';
import { EtaConfig } from 'eta';

// @ts-ignore
type Vars = Record<string, Vars | string>;
// @ts-ignore
type Flags = Record<string, Flags | boolean>;

interface CyanInput {
  vars: Vars;
  flags: Flags;
  parser?: {
    varSyntax?: [string, string][];
    // flagSyntax?: [string, string][],
  };
}

StartProcessorWithLambda(async (input, fileHelper): Promise<ProcessorOutput> => {
  const cfg: CyanInput = input.config as CyanInput;

  const varEtaConfig: Partial<EtaConfig> = {
    useWith: true,
    tags: ['var__', '__'],
    autoTrim: [false, false],
    autoEscape: false,
    parse: {
      raw: '~',
      exec: '=',
      interpolate: '',
    },
  };

  const varSyntax = cfg.parser?.varSyntax ?? [];
  if (varSyntax.length === 0) varSyntax.push(['var__', '__']);

  const varEtaConfigs: Partial<EtaConfig>[] = varSyntax.map(
    s =>
      ({
        ...varEtaConfig,
        tags: s,
      }) satisfies Partial<EtaConfig>,
  );
  const varEtas: Eta[] = varEtaConfigs.map(c => new Eta(c));

  const template = fileHelper.resolveAll();
  template
    .map(x => {
      x.content = varEtas.reduce((acc, eta) => eta.renderString(acc, cfg.vars ?? {}), x.content);
      x.relative = varEtas.reduce((acc, eta) => eta.renderString(acc, cfg.vars ?? {}), x.relative);
      return x;
    })
    .map(x => x.writeFile());

  return { directory: input.writeDir };
});
