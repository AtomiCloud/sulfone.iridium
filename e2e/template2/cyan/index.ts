import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const name = await i.text('What is your name?', 'ernest/template1/name');

  const investmentType = await i.select(
    'What type of investment are you most interested in?',
    ['Stocks', 'Bonds', 'Real Estate', 'Cryptocurrency'],
    'ernest/template2/investmentType',
  );

  const riskTolerance = await i.select(
    'How would you describe your risk tolerance?',
    ['Conservative', 'Moderate', 'Aggressive'],
    'ernest/template2/riskTolerance',
  );

  const savingsGoal = await i.select(
    'What is your primary financial goal?',
    ['Retirement', 'Home Purchase', 'Education', 'Emergency Fund', 'Wealth Building'],
    'ernest/template2/savingsGoal',
  );

  const investmentHorizon = await i.select(
    'What is your investment time horizon?',
    ['Short-term (< 2 years)', 'Medium-term (2-5 years)', 'Long-term (> 5 years)'],
    'ernest/template2/investmentHorizon',
  );

  const incomeSource = await i.select(
    'What is your primary source of income?',
    ['Salary', 'Business', 'Investments', 'Freelance', 'Other'],
    'ernest/template2/incomeSource',
  );

  return {
    processors: [
      {
        name: 'ernest/processor2',
        files: [
          {
            glob: '**/*',
            exclude: [],
            type: GlobType.Template,
            root: 'template',
          },
        ],
        config: {
          vars: {
            name,
            investmentType,
            riskTolerance,
            savingsGoal,
            investmentHorizon,
            incomeSource,
          },
        },
      },
    ],
    plugins: [
      {
        name: 'ernest/plugin2',
        config: {},
      },
    ],
  };
});
